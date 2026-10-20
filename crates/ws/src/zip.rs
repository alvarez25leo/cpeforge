//! ZIP del XML y extracción de CDR, con límites de tamaño y nombres seguros.

use std::io::{Cursor, Read, Write};

use flate2::write::DeflateEncoder;
use flate2::{Compression, Crc};
use zip::ZipArchive;

use crate::WsError;

/// Máximo de bytes de un ZIP de entrada o salida.
pub const MAX_ZIP_BYTES: usize = 16 * 1024 * 1024;
/// Máximo de bytes descomprimidos entre todas las entradas.
pub const MAX_UNCOMPRESSED_BYTES: usize = 32 * 1024 * 1024;
/// Máximo de entradas en un ZIP recibido.
pub const MAX_ENTRIES: usize = 64;

/// Entrada descomprimida en el orden del archivo.
#[derive(Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// Nombre del archivo, sin rutas ascendentes.
    pub filename: String,
    /// Contenido binario.
    pub content: Vec<u8>,
}

impl std::fmt::Debug for ZipEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZipEntry")
            .field("filename", &self.filename)
            .field("content_bytes", &self.content.len())
            .finish()
    }
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && !name.contains(':')
        && name.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

/// Nivel DEFLATE de la entrada. Forma parte del contrato de bytes del ZIP:
/// cambiarlo cambia el `hashZip` que se envía por GRE.
const DEFLATE_LEVEL: u32 = 5;
/// «Version made by / needed»: Unix (3) y versión 2.0.
const VERSION_UNIX_20: u16 = 0x0314;
/// Bit 11 del indicador general: nombre en UTF-8, solo para nombres no ASCII.
const FLAG_UTF8: u16 = 0x0800;
/// Modo Unix `0100644` con el atributo DOS de archivo (`0x20`).
const EXTERNAL_ATTRIBUTES: u32 = 0o100_644 << 16 | 0x20;
/// Posición del tamaño comprimido en la cabecera local.
const LOCAL_COMPRESSED_SIZE_AT: usize = 18;

/// Fecha y hora MS-DOS de la entrada ZIP, en hora local sin zona.
///
/// Los segundos se redondean hacia abajo a número par, como exige el formato
/// MS-DOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DosDateTime {
    time: u16,
    date: u16,
}

impl DosDateTime {
    /// Valor que se escribe para instantes anteriores o iguales a
    /// 1980-01-01 00:00:01 (fecha y hora a cero).
    pub const MIN: Self = Self { time: 0, date: 0 };

    /// Construye desde componentes civiles. `None` si están fuera del rango
    /// MS-DOS (años 1980–2107) o no forman una fecha y hora válidas.
    #[must_use]
    pub fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Self> {
        let year_offset = u16::try_from(year.checked_sub(1980)?).ok()?;
        if year_offset > 127
            || !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
            || hour > 23
            || minute > 59
            || second > 59
        {
            return None;
        }
        let value = Self {
            time: u16::from(hour) << 11 | u16::from(minute) << 5 | u16::from(second >> 1),
            date: year_offset << 9 | u16::from(month) << 5 | u16::from(day),
        };
        // El primer instante representable (1980-01-01 00:00:00/01) se escribe
        // como fecha y hora a cero.
        Some(if value.date == 0x21 && value.time == 0 {
            Self::MIN
        } else {
            value
        })
    }

    /// Hora civil de `unix_seconds + utc_offset_seconds`. Antes de 1980 da
    /// [`Self::MIN`]; después de 2107 satura en 2107-12-31 23:59:58.
    #[must_use]
    pub fn from_unix(unix_seconds: i64, utc_offset_seconds: i32) -> Self {
        let local = unix_seconds.saturating_add(i64::from(utc_offset_seconds));
        let days = local.div_euclid(86_400);
        let second_of_day = local.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        let clock = |divisor: i64, modulus: i64| {
            u8::try_from(second_of_day / divisor % modulus).unwrap_or_default()
        };
        if year < 1980 {
            return Self::MIN;
        }
        Self::new(
            year,
            month,
            day,
            clock(3600, 24),
            clock(60, 60),
            clock(1, 60),
        )
        .unwrap_or(Self {
            time: 23 << 11 | 59 << 5 | 29,
            date: 127 << 9 | 12 << 5 | 31,
        })
    }

    /// Lee los campos de una cabecera ZIP (hora y fecha MS-DOS).
    #[must_use]
    pub const fn from_fields(time: u16, date: u16) -> Self {
        Self { time, date }
    }

    /// Campos `(last mod file time, last mod file date)`.
    #[must_use]
    pub const fn fields(self) -> (u16, u16) {
        (self.time, self.date)
    }
}

/// Instante que se escribe en la entrada ZIP.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ZipTime {
    /// Hora actual del sistema en UTC.
    #[default]
    SystemUtc,
    /// Hora actual del sistema con desplazamiento fijo en segundos (por
    /// ejemplo, `-18_000` para `America/Lima`).
    SystemOffset(i32),
    /// Fecha y hora fijas, para salidas reproducibles.
    Fixed(DosDateTime),
}

impl ZipTime {
    fn resolve(self) -> DosDateTime {
        let now = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| {
                    i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
                })
        };
        match self {
            Self::SystemUtc => DosDateTime::from_unix(now(), 0),
            Self::SystemOffset(offset) => DosDateTime::from_unix(now(), offset),
            Self::Fixed(value) => value,
        }
    }
}

fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Algoritmo `civil_from_days` de Howard Hinnant (calendario gregoriano
/// proléptico).
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        i32::try_from(year).unwrap_or(if year < 0 { i32::MIN } else { i32::MAX }),
        u8::try_from(month).unwrap_or_default(),
        u8::try_from(day).unwrap_or_default(),
    )
}

/// Comprime una entrada DEFLATE con la hora actual en UTC. Equivale a
/// [`compress_at`] con [`ZipTime::SystemUtc`].
///
/// # Errors
///
/// Rechaza nombre inseguro, exceso de tamaño o fallo del codificador ZIP.
pub fn compress(filename: &str, content: &[u8]) -> Result<Vec<u8>, WsError> {
    compress_at(filename, content, ZipTime::SystemUtc)
}

/// Comprime una entrada DEFLATE: zlib nivel 5, versión 2.0 Unix, modo
/// `0644` con atributo de archivo DOS, sin campos extra ni descriptor de
/// datos. La salida es determinista para el mismo nombre, contenido y hora.
///
/// El ZIP resultante es el que debe usarse tanto para Base64 como para
/// SHA-256 al enviar por GRE. El flujo DEFLATE se escribe directamente tras
/// la cabecera local, en el mismo buffer que el resto del archivo.
///
/// # Errors
///
/// Rechaza nombre inseguro, exceso de tamaño o fallo del codificador ZIP.
pub fn compress_at(filename: &str, content: &[u8], time: ZipTime) -> Result<Vec<u8>, WsError> {
    if !safe_name(filename) {
        return Err(WsError::Security("nombre ZIP inseguro"));
    }
    if content.len() > MAX_UNCOMPRESSED_BYTES {
        return Err(WsError::Zip("entrada demasiado grande"));
    }
    let name = filename.as_bytes();
    let name_len = u16::try_from(name.len()).map_err(|_| WsError::Zip("nombre demasiado largo"))?;
    let mut crc = Crc::new();
    crc.update(content);
    let crc = crc.sum();
    let uncompressed =
        u32::try_from(content.len()).map_err(|_| WsError::Zip("entrada demasiado grande"))?;
    let flags = if filename.is_ascii() { 0 } else { FLAG_UTF8 };
    let (dos_time, dos_date) = time.resolve().fields();

    // Un XML comprime a menos de una octava parte; si no, el vector crece.
    let mut out = Vec::with_capacity(128 + 2 * name.len() + content.len() / 8);
    // Cabecera local; el tamaño comprimido se rellena al terminar DEFLATE.
    put32(&mut out, 0x0403_4b50);
    put16(&mut out, VERSION_UNIX_20);
    put16(&mut out, flags);
    put16(&mut out, 8);
    put16(&mut out, dos_time);
    put16(&mut out, dos_date);
    put32(&mut out, crc);
    put32(&mut out, 0);
    put32(&mut out, uncompressed);
    put16(&mut out, name_len);
    put16(&mut out, 0);
    out.extend_from_slice(name);
    let data_start = out.len();
    let mut out = deflate_into(out, content).map_err(|_| WsError::Zip("no se pudo comprimir"))?;
    let compressed =
        u32::try_from(out.len() - data_start).map_err(|_| WsError::Zip("ZIP demasiado grande"))?;
    if let Some(field) = out.get_mut(LOCAL_COMPRESSED_SIZE_AT..LOCAL_COMPRESSED_SIZE_AT + 4) {
        field.copy_from_slice(&compressed.to_le_bytes());
    }
    // Directorio central.
    let central_offset =
        u32::try_from(out.len()).map_err(|_| WsError::Zip("ZIP demasiado grande"))?;
    put32(&mut out, 0x0201_4b50);
    put16(&mut out, VERSION_UNIX_20);
    put16(&mut out, VERSION_UNIX_20);
    put16(&mut out, flags);
    put16(&mut out, 8);
    put16(&mut out, dos_time);
    put16(&mut out, dos_date);
    put32(&mut out, crc);
    put32(&mut out, compressed);
    put32(&mut out, uncompressed);
    put16(&mut out, name_len);
    put16(&mut out, 0); // extra
    put16(&mut out, 0); // comentario
    put16(&mut out, 0); // disco
    put16(&mut out, 0); // atributos internos
    put32(&mut out, EXTERNAL_ATTRIBUTES);
    put32(&mut out, 0); // desplazamiento de la cabecera local
    out.extend_from_slice(name);
    let central_size = u32::try_from(out.len())
        .map_err(|_| WsError::Zip("ZIP demasiado grande"))?
        .saturating_sub(central_offset);
    // Fin del directorio central.
    put32(&mut out, 0x0605_4b50);
    put16(&mut out, 0);
    put16(&mut out, 0);
    put16(&mut out, 1);
    put16(&mut out, 1);
    put32(&mut out, central_size);
    put32(&mut out, central_offset);
    put16(&mut out, 0);
    if out.len() > MAX_ZIP_BYTES {
        return Err(WsError::Zip("ZIP demasiado grande"));
    }
    // La reserva inicial es una estimación: no se retiene el sobrante en un
    // buffer que después se codifica en Base64 y se hashea.
    if out.capacity() - out.len() > out.len() / 8 {
        out.shrink_to_fit();
    }
    Ok(out)
}

/// Añade a `out` el flujo DEFLATE de `content` y devuelve el buffer.
fn deflate_into(out: Vec<u8>, content: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(out, Compression::new(DEFLATE_LEVEL));
    encoder.write_all(content)?;
    encoder.finish()
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Descomprime todas las entradas en orden sin escribir al sistema de archivos.
///
/// # Errors
///
/// Rechaza ZIP inválido, traversal, bombas y exceso de entradas/tamaño.
pub fn decompress(bytes: &[u8]) -> Result<Vec<ZipEntry>, WsError> {
    read_entries(bytes, |_| true)
}

/// Recorre todas las entradas en orden, validando nombre, tamaño y CRC de
/// cada una. Solo conserva el contenido de las que `keep` acepta; las demás
/// se descomprimen sin guardarlas, de modo que una entrada corrupta o fuera
/// de límites se rechaza igual aunque no se devuelva.
fn read_entries(
    bytes: &[u8],
    mut keep: impl FnMut(&str) -> bool,
) -> Result<Vec<ZipEntry>, WsError> {
    if bytes.len() > MAX_ZIP_BYTES {
        return Err(WsError::Zip("ZIP demasiado grande"));
    }
    if !declared_entries_fit(bytes) {
        return Err(WsError::Zip("demasiadas entradas"));
    }
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|_| WsError::Zip("estructura ZIP inválida"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(WsError::Zip("demasiadas entradas"));
    }
    let mut total = 0usize;
    let mut result = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|_| WsError::Zip("entrada ZIP inválida"))?;
        let filename = file
            .name()
            .map_err(|_| WsError::Zip("nombre ZIP inválido"))?
            .into_owned();
        // Las entradas de directorio terminan en `/`: el CDR de SUNAT trae
        // `dummy/`. Se devuelven vacías; nada se escribe en disco.
        if !safe_name(filename.strip_suffix('/').unwrap_or(&filename)) {
            return Err(WsError::Security("nombre ZIP inseguro"));
        }
        let remaining = MAX_UNCOMPRESSED_BYTES.saturating_sub(total);
        if file.size() > remaining as u64 {
            return Err(WsError::Zip("contenido descomprimido demasiado grande"));
        }
        let limit = (remaining as u64).saturating_add(1);
        let read = if keep(&filename) {
            // El tamaño declarado ya está acotado por `remaining`; reservarlo
            // evita copias al crecer. Si la cabecera exagera, se libera el
            // sobrante.
            let declared = usize::try_from(file.size()).unwrap_or(remaining);
            let mut content = Vec::with_capacity(declared.min(remaining));
            file.by_ref()
                .take(limit)
                .read_to_end(&mut content)
                .map_err(|_| WsError::Zip("no se pudo descomprimir"))?;
            if content.len() < content.capacity() {
                content.shrink_to_fit();
            }
            let read = content.len();
            result.push(ZipEntry { filename, content });
            read
        } else {
            let read = std::io::copy(&mut file.by_ref().take(limit), &mut std::io::sink())
                .map_err(|_| WsError::Zip("no se pudo descomprimir"))?;
            usize::try_from(read).unwrap_or(usize::MAX)
        };
        total = total.saturating_add(read);
        if total > MAX_UNCOMPRESSED_BYTES {
            return Err(WsError::Zip("contenido descomprimido demasiado grande"));
        }
    }
    Ok(result)
}

/// Firma del registro de fin del directorio central (EOCD).
const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
/// Bytes fijos del EOCD sin la firma.
const EOCD_FIELDS: usize = 18;

/// Indica si ningún registro de fin de directorio central que el lector
/// podría elegir declara más de [`MAX_ENTRIES`] entradas ni marca ZIP64.
///
/// `ZipArchive::new` lee todas las entradas que declara el registro antes de
/// que se pueda contar `archive.len()`: un ZIP de 16 MiB declara decenas de
/// miles. El lector prueba varios candidatos (cualquier firma `PK\x05\x06`
/// cuyo comentario termine dentro del archivo), así que se comprueban todos:
/// si se aceptara uno y el lector eligiera otro, el límite se saltaría. Sin
/// los valores `0xFFFF`/`0xFFFFFFFF` que activan ZIP64, el lector usa los
/// recuentos de 16 bits ya comprobados.
fn declared_entries_fit(bytes: &[u8]) -> bool {
    let mut from = 0;
    while let Some(offset) = bytes.get(from..).and_then(|rest| {
        rest.windows(EOCD_SIGNATURE.len())
            .position(|w| w == EOCD_SIGNATURE)
    }) {
        let at = from + offset;
        from = at + 1;
        let start = at + EOCD_SIGNATURE.len();
        let Some(fields) = bytes.get(start..start + EOCD_FIELDS) else {
            continue;
        };
        let u16_at = |i: usize| u16::from_le_bytes([fields[i], fields[i + 1]]);
        let u32_at =
            |i: usize| u32::from_le_bytes([fields[i], fields[i + 1], fields[i + 2], fields[i + 3]]);
        // Un comentario que pasa del final descarta el candidato.
        if start + EOCD_FIELDS + usize::from(u16_at(16)) > bytes.len() {
            continue;
        }
        let entries_on_disk = usize::from(u16_at(4));
        let entries = usize::from(u16_at(6));
        if entries_on_disk > MAX_ENTRIES
            || entries > MAX_ENTRIES
            || u32_at(8) == u32::MAX
            || u32_at(12) == u32::MAX
        {
            return false;
        }
    }
    true
}

/// Contenido de la primera entrada con extensión `.xml` (sin distinguir
/// mayúsculas), en el orden del archivo.
///
/// Todas las entradas se validan como en [`decompress`], pero solo se
/// conserva en memoria la seleccionada.
///
/// # Errors
///
/// Propaga fallo de ZIP; `None` cuando no hay XML.
pub fn first_xml(bytes: &[u8]) -> Result<Option<Vec<u8>>, WsError> {
    let mut found = false;
    let entries = read_entries(bytes, |name| {
        let selected = !found && is_xml(name);
        found |= selected;
        selected
    })?;
    Ok(entries.into_iter().next().map(|entry| entry.content))
}

fn is_xml(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("xml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_traversal() {
        let zipped = compress("A.xml", b"<A/>").unwrap_or_default();
        assert_eq!(zipped.get(4..6), Some(&[0x14, 0x03][..]));
        assert_eq!(first_xml(&zipped), Ok(Some(b"<A/>".to_vec())));
        assert!(matches!(
            compress("../escape.xml", b"x"),
            Err(WsError::Security(_))
        ));
        assert!(matches!(decompress(b"not a zip"), Err(WsError::Zip(_))));
    }

    #[test]
    fn dos_date_time() {
        // 2026-10-17T20:31:20Z en UTC.
        assert_eq!(
            DosDateTime::from_unix(1_792_269_080, 0).fields(),
            (0xa3ea, 0x5d51)
        );
        assert_eq!(
            DosDateTime::from_unix(1_792_269_080, -18_000),
            DosDateTime::new(2026, 10, 17, 15, 31, 20).unwrap_or(DosDateTime::MIN)
        );
        assert_eq!(DosDateTime::from_unix(0, 0), DosDateTime::MIN);
        assert_eq!(
            DosDateTime::new(1980, 1, 1, 0, 0, 1),
            Some(DosDateTime::MIN)
        );
        assert_eq!(DosDateTime::new(2024, 2, 30, 0, 0, 0), None);
        assert_eq!(DosDateTime::new(2108, 1, 1, 0, 0, 0), None);
        assert_eq!(
            DosDateTime::from_unix(i64::MAX, 0).fields(),
            (23 << 11 | 59 << 5 | 29, 127 << 9 | 12 << 5 | 31)
        );
        assert_eq!(DosDateTime::from_unix(i64::MIN, 0), DosDateTime::MIN);
    }

    /// EOCD con `entries` entradas, tamaño y desplazamiento del directorio y
    /// comentario de `comment` bytes declarados.
    fn eocd(entries: u16, size: u32, offset: u32, comment: u16) -> Vec<u8> {
        let mut out = EOCD_SIGNATURE.to_vec();
        for value in [0u16, 0, entries, entries] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&comment.to_le_bytes());
        out
    }

    #[test]
    fn declared_entries_are_checked_on_every_candidate() {
        let zipped = compress("A.xml", b"<A/>").unwrap_or_default();
        assert!(declared_entries_fit(&zipped));
        // Más entradas que el máximo, o ZIP64.
        assert!(!declared_entries_fit(&eocd(65, 0, 0, 0)));
        assert!(!declared_entries_fit(&eocd(1, u32::MAX, 0, 0)));
        assert!(!declared_entries_fit(&eocd(1, 0, u32::MAX, 0)));
        // Un candidato válido al final no oculta uno anterior que el lector
        // también podría elegir.
        let mut two = eocd(60_000, 0, 0, 22);
        two.extend(eocd(1, 0, 0, 0));
        assert!(!declared_entries_fit(&two));
        // Un candidato cuyo comentario pasa del final no cuenta.
        assert!(declared_entries_fit(&eocd(60_000, 0, 0, 1)));
        assert!(declared_entries_fit(&EOCD_SIGNATURE));
        assert_eq!(
            decompress(&eocd(60_000, 0, 0, 0)),
            Err(WsError::Zip("demasiadas entradas"))
        );
    }

    #[test]
    fn utf8_flag_only_for_non_ascii_names() {
        let fixed = ZipTime::Fixed(DosDateTime::MIN);
        let ascii = compress_at("A.xml", b"x", fixed).unwrap_or_default();
        let utf8 = compress_at("Ñ.xml", b"x", fixed).unwrap_or_default();
        assert_eq!(ascii.get(6..8), Some(&[0, 0][..]));
        assert_eq!(utf8.get(6..8), Some(&[0, 0x08][..]));
        assert_eq!(first_xml(&utf8), Ok(Some(b"x".to_vec())));
    }
}
