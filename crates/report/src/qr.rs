//! Código QR en SVG: 120 × 120 píxeles, sin margen y con corrección Q.
//!
//! El símbolo y los bytes del SVG están fijados por el corpus de
//! `tests/reference/report`; cada decisión de abajo existe para reproducirlos.
//!
//! - **Codificación:** [`qrcodegen`] construye la matriz (segmentos,
//!   Reed–Solomon, patrones y formato). Se usa un **único** segmento
//!   (numérico, alfanumérico o byte) y, en modo byte, el prefijo ECI 26
//!   (UTF-8). La versión se elige con un byte de margen (ver
//!   `select_version`).
//! - **Máscara:** las ocho máscaras se puntúan con la variante ZXing de las
//!   reglas N1–N4, que no siempre coincide con la de `qrcodegen`, y se
//!   elige la primera de menor puntuación. Solo la máscara 0 se codifica:
//!   las otras siete se obtienen invirtiendo los módulos que cambian con la
//!   máscara, una tabla por versión que `qrcodegen` calcula una vez y que no
//!   depende del contenido (ver `mask_deltas`).
//! - **SVG:** un único `path` par-impar con el contorno de los módulos
//!   oscuros (trazado de bordes), los tres ojos dibujados aparte y rotados, y
//!   coordenadas redondeadas a tres decimales.
//!
//! No contiene una implementación propia del algoritmo QR. Las funciones son
//! puras y seguras entre hilos; la única memoria compartida son las tablas
//! por versión, de tamaño acotado e inmutables una vez creadas.

use std::fmt;
use std::fmt::Write as _;
use std::sync::{LazyLock, OnceLock};

use qrcodegen::{DataTooLong, Mask, QrCode, QrCodeEcc, QrSegment, Version};

use crate::format;

/// Tamaño del SVG en píxeles; el símbolo no lleva margen.
pub const SVG_SIZE: u32 = 120;

/// Caracteres del modo alfanumérico.
const ALPHANUMERIC: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/// Designador ECI de UTF-8.
const ECI_UTF8: u32 = 26;

/// Palabras de datos por versión con corrección Q (ISO/IEC 18004, tabla 7).
/// La prueba `capacity_table_matches_qrcodegen` la contrasta con `qrcodegen`.
const DATA_CODEWORDS_Q: [u32; 40] = [
    13, 22, 34, 48, 62, 76, 88, 110, 132, 154, 180, 206, 244, 261, 295, 325, 367, 397, 445, 485,
    512, 568, 614, 664, 718, 754, 808, 871, 911, 985, 1033, 1115, 1171, 1231, 1286, 1354, 1426,
    1502, 1582, 1666,
];

/// Fallo al generar el código QR.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QrError {
    /// Contenido vacío (`InvalidArgumentException: Found empty contents`).
    EmptyContent,
    /// El contenido no cabe en la versión 40 con nivel Q
    /// (`WriterException: Data too big`).
    DataTooBig,
    /// Bytes que no son UTF-8 válido
    /// (`WriterException: Could not encode content to UTF-8`).
    InvalidUtf8,
}

impl fmt::Display for QrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyContent => "Found empty contents",
            Self::DataTooBig => "Data too big",
            Self::InvalidUtf8 => "Could not encode content to UTF-8",
        })
    }
}

impl std::error::Error for QrError {}

/// Modo de codificación elegido para todo el contenido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum QrMode {
    /// Solo dígitos.
    Numeric,
    /// Solo dígitos, mayúsculas, espacio y `$%*+-./:`.
    Alphanumeric,
    /// Bytes UTF-8 con prefijo ECI 26.
    Byte,
}

/// Símbolo QR ya codificado.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct QrSymbol {
    /// Modo del único segmento de datos.
    pub mode: QrMode,
    /// Versión (1–40).
    pub version: u8,
    /// Máscara (0–7).
    pub mask: u8,
    /// Módulos por fila, `true` = oscuro; `size × size`.
    pub modules: Vec<Vec<bool>>,
}

impl QrSymbol {
    /// Lado de la matriz en módulos.
    #[must_use]
    pub fn size(&self) -> usize {
        self.modules.len()
    }
}

fn choose_mode(data: &[u8]) -> QrMode {
    if data.iter().all(u8::is_ascii_digit) {
        QrMode::Numeric
    } else if data.iter().all(|b| ALPHANUMERIC.contains(b)) {
        QrMode::Alphanumeric
    } else {
        QrMode::Byte
    }
}

/// Bits del indicador de longitud (tabla 3 de ISO/IEC 18004).
fn count_bits(mode: QrMode, version: u8) -> u32 {
    let band = match version {
        1..=9 => 0,
        10..=26 => 1,
        _ => 2,
    };
    match mode {
        QrMode::Numeric => [10, 12, 14][band],
        QrMode::Alphanumeric => [9, 11, 13][band],
        QrMode::Byte => [8, 16, 16][band],
    }
}

/// Capacidad numérica de la versión 40 con corrección Q (dígitos).
const MAX_CONTENT_BYTES: usize = 3993;

fn data_bits(mode: QrMode, len: u32) -> u32 {
    match mode {
        QrMode::Numeric => len / 3 * 10 + [0, 4, 7][usize::try_from(len % 3).unwrap_or(0)],
        QrMode::Alphanumeric => len / 2 * 11 + (len % 2) * 6,
        QrMode::Byte => len * 8,
    }
}

/// Primera versión con `bytes de datos >= (bits + 8) / 8`. Exige un byte más
/// que la capacidad estricta cuando los bits son múltiplo de 8, así que a
/// veces elige una versión mayor que la mínima de `qrcodegen` o rechaza un
/// contenido que cabría.
fn choose_version(bits: u32) -> Option<u8> {
    let needed = (bits + 8) / 8;
    DATA_CODEWORDS_Q
        .iter()
        .position(|bytes| *bytes >= needed)
        .and_then(|index| u8::try_from(index + 1).ok())
}

/// Versión del símbolo: estimación con el indicador de longitud de la
/// versión 1 y recálculo con la versión estimada.
fn select_version(mode: QrMode, len: usize) -> Option<u8> {
    let len = u32::try_from(len).ok()?;
    let header = if mode == QrMode::Byte { 4 + 8 } else { 0 } + 4;
    let data = data_bits(mode, len);
    let provisional = choose_version(header + count_bits(mode, 1) + data)?;
    choose_version(header + count_bits(mode, provisional) + data)
}

/// Codifica `content` (UTF-8) con corrección Q.
///
/// # Errors
///
/// [`QrError::EmptyContent`] con contenido vacío, [`QrError::InvalidUtf8`]
/// si no es UTF-8 y [`QrError::DataTooBig`] si no cabe en la versión 40.
pub fn encode(content: &[u8]) -> Result<QrSymbol, QrError> {
    if content.is_empty() {
        return Err(QrError::EmptyContent);
    }
    // Nada mayor que la capacidad numérica de la versión 40-Q cabe: se
    // rechaza antes de construir los segmentos (un bit por `bool`).
    if content.len() > MAX_CONTENT_BYTES {
        return Err(QrError::DataTooBig);
    }
    let text = std::str::from_utf8(content).map_err(|_| QrError::InvalidUtf8)?;
    let mode = choose_mode(content);
    let segments = match mode {
        QrMode::Numeric => vec![QrSegment::make_numeric(text)],
        QrMode::Alphanumeric => vec![QrSegment::make_alphanumeric(text)],
        QrMode::Byte => vec![
            QrSegment::make_eci(ECI_UTF8),
            QrSegment::make_bytes(content),
        ],
    };
    let ecc = QrCodeEcc::Quartile;
    let length = if mode == QrMode::Byte {
        content.len()
    } else {
        text.chars().count()
    };
    let version = Version::new(select_version(mode, length).ok_or(QrError::DataTooBig)?);
    let encode_with = |mask: u8| {
        QrCode::encode_segments_advanced(
            &segments,
            ecc,
            version,
            version,
            Some(Mask::new(mask)),
            false,
        )
        .map_err(|_: DataTooLong| QrError::DataTooBig)
    };
    // Una sola codificación real (máscara 0); las otras siete matrices se
    // obtienen con las diferencias por versión de `mask_deltas`.
    let base = Grid::of(&encode_with(0)?);
    let deltas = mask_deltas(version);
    let variant = |mask: u8| -> Result<Grid, QrError> {
        match (mask, deltas) {
            (0, _) => Ok(base.clone()),
            (_, Some(deltas)) => Ok(base.xor(&deltas.words[usize::from(mask - 1)])),
            (_, None) => Ok(Grid::of(&encode_with(mask)?)),
        }
    };
    let mut best: Option<(u32, u8)> = None;
    for mask in 0..8u8 {
        let penalty = if mask == 0 {
            mask_penalty(&base)
        } else {
            mask_penalty(&variant(mask)?)
        };
        // En caso de empate gana la máscara de número menor.
        if best.is_none_or(|(p, _)| penalty < p) {
            best = Some((penalty, mask));
        }
    }
    let (_, mask) = best.ok_or(QrError::DataTooBig)?;
    Ok(QrSymbol {
        mode,
        version: version.value(),
        mask,
        modules: variant(mask)?.rows(),
    })
}

/// Diferencias (XOR) entre la matriz con la máscara `k` (1–7) y la matriz
/// con la máscara 0, en bits por fila mayor.
///
/// Con versión y nivel de corrección fijos, los patrones de función y la
/// posición de cada bit de datos no cambian con la máscara: solo cambian el
/// patrón de máscara (en los módulos de datos) y los bits de formato. Por eso
/// la diferencia **no depende del contenido** y se calcula una vez por
/// versión, con el propio `qrcodegen` y un contenido vacío.
struct MaskDeltas {
    words: [Vec<u64>; 7],
}

/// Una tabla por versión, creada al primer uso: a lo sumo 40 entradas
/// (≈ 350 KB si se usaran todas las versiones).
static MASK_DELTAS: [OnceLock<Option<MaskDeltas>>; 40] = [const { OnceLock::new() }; 40];

fn mask_deltas(version: Version) -> Option<&'static MaskDeltas> {
    let slot = MASK_DELTAS.get(usize::from(version.value()).checked_sub(1)?)?;
    slot.get_or_init(|| {
        let encode = |mask: u8| {
            QrCode::encode_segments_advanced(
                &[],
                QrCodeEcc::Quartile,
                version,
                version,
                Some(Mask::new(mask)),
                false,
            )
            .ok()
            .map(|code| Grid::of(&code))
        };
        let base = encode(0)?;
        let mut words: [Vec<u64>; 7] = Default::default();
        for (slot, mask) in words.iter_mut().zip(1..8u8) {
            let other = encode(mask)?;
            let mut bits = vec![0u64; base.cells.len().div_ceil(64)];
            for (index, (a, b)) in base.cells.iter().zip(&other.cells).enumerate() {
                if a != b {
                    bits[index / 64] |= 1 << (index % 64);
                }
            }
            *slot = bits;
        }
        Some(MaskDeltas { words })
    })
    .as_ref()
}

/// Matriz cuadrada de módulos por filas, con su traspuesta para recorrer
/// las columnas como segmentos contiguos.
#[derive(Clone)]
struct Grid {
    size: usize,
    cells: Vec<bool>,
    transposed: Vec<bool>,
}

impl Grid {
    fn of(code: &QrCode) -> Self {
        let side = code.size();
        let size = usize::try_from(side).unwrap_or(0);
        let mut cells = Vec::with_capacity(size * size);
        for y in 0..side {
            for x in 0..side {
                cells.push(code.get_module(x, y));
            }
        }
        Self::from_cells(size, cells)
    }

    fn from_cells(size: usize, cells: Vec<bool>) -> Self {
        let mut transposed = vec![false; cells.len()];
        for (y, row) in Self::lines(&cells, size).enumerate() {
            for (x, cell) in row.iter().enumerate() {
                transposed[x * size + y] = *cell;
            }
        }
        Self {
            size,
            cells,
            transposed,
        }
    }

    /// Esta matriz con los módulos de `delta` invertidos.
    fn xor(&self, delta: &[u64]) -> Self {
        let mut cells = self.cells.clone();
        for (chunk, word) in cells.chunks_mut(64).zip(delta) {
            if *word == 0 {
                continue;
            }
            for (bit, cell) in chunk.iter_mut().enumerate() {
                *cell ^= (word >> bit) & 1 == 1;
            }
        }
        Self::from_cells(self.size, cells)
    }

    /// Filas (`cells`) o columnas (`transposed`) como segmentos.
    fn lines(cells: &[bool], size: usize) -> std::slice::Chunks<'_, bool> {
        cells.chunks(size.max(1))
    }

    fn rows(&self) -> Vec<Vec<bool>> {
        Self::lines(&self.cells, self.size)
            .map(<[bool]>::to_vec)
            .collect()
    }
}

/// Puntuación de una máscara: reglas N1–N4 en la variante de ZXing.
fn mask_penalty(m: &Grid) -> u32 {
    rule1(m) + rule2(m) + rule3(m) + rule4(m)
}

/// N1: tramos de 5 o más módulos iguales en filas y columnas; cada tramo de
/// longitud `L` suma `3 + (L - 5)`.
fn rule1(m: &Grid) -> u32 {
    let mut penalty = 0usize;
    for cells in [&m.cells, &m.transposed] {
        for line in Grid::lines(cells, m.size) {
            penalty += rule1_line(line);
        }
    }
    u32::try_from(penalty).unwrap_or(u32::MAX)
}

/// N1 de una línea sin recorrer los tramos uno a uno: un tramo de longitud
/// `L ≥ 5` contiene `L - 4` ventanas de cinco módulos iguales y empieza una
/// sola vez, así que `L - 2 = (L - 4) + 2`. Ambos conteos se vectorizan.
fn rule1_line(line: &[bool]) -> usize {
    let Some(n) = line.len().checked_sub(4) else {
        return 0;
    };
    let shifted = |k: usize| &line[k..k + n];
    let (a, b, c, d, e) = (shifted(0), shifted(1), shifted(2), shifted(3), shifted(4));
    let mut windows = 0usize;
    let mut starts = 0usize;
    for i in 0..n {
        let equal = a[i] == b[i] && a[i] == c[i] && a[i] == d[i] && a[i] == e[i];
        let first = i == 0 || a[i - 1] != a[i];
        windows += usize::from(equal);
        starts += usize::from(equal && first);
    }
    windows + 2 * starts
}

/// N2: bloques 2×2 del mismo color.
fn rule2(m: &Grid) -> u32 {
    let mut count = 0usize;
    let mut rows = Grid::lines(&m.cells, m.size);
    let Some(mut top) = rows.next() else {
        return 0;
    };
    for bottom in rows {
        count += adjacent_pairs(top)
            .zip(adjacent_pairs(bottom))
            .filter(|&((a, b), (c, d))| a == b && a == c && a == d)
            .count();
        top = bottom;
    }
    3 * u32::try_from(count).unwrap_or(u32::MAX)
}

/// Pares de módulos vecinos de una fila: `(x, x + 1)`.
fn adjacent_pairs(row: &[bool]) -> impl Iterator<Item = (bool, bool)> + '_ {
    row.iter().zip(row.iter().skip(1)).map(|(a, b)| (*a, *b))
}

/// N3: patrón 1011101 con cuatro módulos claros antes o después.
fn rule3(m: &Grid) -> u32 {
    const PATTERN: [bool; 7] = [true, false, true, true, true, false, true];
    let light = |cells: &[bool]| cells.iter().all(|b| !*b);
    let mut penalty = 0u32;
    for cells in [&m.cells, &m.transposed] {
        for line in Grid::lines(cells, m.size) {
            let n = line.len();
            for x in 0..n.saturating_sub(6) {
                if line[x..x + 7] == PATTERN
                    && ((x + 10 < n && light(&line[x + 7..x + 11]))
                        || (x >= 4 && light(&line[x - 4..x])))
                {
                    penalty += 40;
                }
            }
        }
    }
    penalty
}

#[allow(
    clippy::float_arithmetic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "N4 se calcula en coma flotante con redondeo hacia abajo"
)]
/// N4: desviación de la proporción de módulos oscuros respecto del 50 %.
fn rule4(m: &Grid) -> u32 {
    let dark = m.cells.iter().filter(|b| **b).count();
    let total = m.cells.len();
    let ratio = to_f64(to_i64(dark)) / to_f64(to_i64(total));
    let variances = ((ratio - 0.5).abs() * 20.0).floor() as u32;
    variances * 10
}

/// Contenido del QR de venta: `RUC|tipo|serie|correlativo|IGV|total|fecha|tipoDocCliente|numDocCliente|`.
///
/// Los importes van con dos decimales ([`crate::format::number_format`]) y la
/// fecha como `Y-m-d` en su propia zona horaria (no se convierte a Lima).
#[must_use]
pub fn sale_payload(parts: &SalePayloadParts<'_>) -> String {
    let mut out = String::new();
    for value in [
        parts.ruc,
        parts.tipo_doc,
        parts.serie,
        parts.correlativo,
        Some(parts.igv.as_str()),
        Some(parts.total.as_str()),
        Some(parts.fecha.as_str()),
        parts.client_tipo_doc,
        parts.client_num_doc,
    ] {
        out.push_str(value.unwrap_or_default());
        out.push('|');
    }
    out
}

/// Partes ya formateadas del contenido QR de venta.
#[derive(Debug, Clone)]
pub struct SalePayloadParts<'a> {
    /// RUC del emisor.
    pub ruc: Option<&'a str>,
    /// Tipo de comprobante.
    pub tipo_doc: Option<&'a str>,
    /// Serie.
    pub serie: Option<&'a str>,
    /// Correlativo.
    pub correlativo: Option<&'a str>,
    /// IGV con dos decimales.
    pub igv: String,
    /// Importe total con dos decimales.
    pub total: String,
    /// Fecha `Y-m-d`.
    pub fecha: String,
    /// Tipo de documento del cliente.
    pub client_tipo_doc: Option<&'a str>,
    /// Número de documento del cliente.
    pub client_num_doc: Option<&'a str>,
}

/// SVG del código QR de `content`.
///
/// # Errors
///
/// Los de [`encode`].
pub fn svg(content: &[u8]) -> Result<String, QrError> {
    Ok(svg_from_symbol(&encode(content)?))
}

/// SVG de un símbolo ya codificado.
#[must_use]
pub fn svg_from_symbol(symbol: &QrSymbol) -> String {
    let size = symbol.size();
    let mut bytes: Vec<u8> = symbol
        .modules
        .iter()
        .flat_map(|row| row.iter().map(|b| u8::from(*b)))
        .collect();
    remove_position_patterns(&mut bytes, size);
    let mut ops = module_path(bytes, size, size);
    append_eyes(&mut ops, size);

    #[allow(clippy::float_arithmetic, reason = "escala del SVG")]
    let module_size = f64::from(SVG_SIZE) / f64::from(u32::try_from(size).unwrap_or(1));
    // Cabecera y pie (~330 bytes) y unos 9 bytes por operación del trazado.
    let mut out = String::with_capacity(512 + ops.len() * 9);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{SVG_SIZE}\" height=\"{SVG_SIZE}\" viewBox=\"0 0 {SVG_SIZE} {SVG_SIZE}\">"
    );
    let _ = write!(
        out,
        "<rect x=\"0\" y=\"0\" width=\"{SVG_SIZE}\" height=\"{SVG_SIZE}\" fill=\"#ffffff\"/>"
    );
    out.push_str("<g transform=\"scale(");
    write_coord(&mut out, module_size);
    out.push_str(")\"><g transform=\"translate(");
    write_coord(&mut out, 0.0);
    out.push(',');
    write_coord(&mut out, 0.0);
    out.push_str(")\">");
    out.push_str("<path fill-rule=\"evenodd\" d=\"");
    for op in &ops {
        let (command, x, y) = match *op {
            PathOp::Move(x, y) => ('M', x, y),
            PathOp::Line(x, y) => ('L', x, y),
            PathOp::Close => {
                out.push('Z');
                continue;
            }
        };
        out.push(command);
        write_coord(&mut out, x);
        out.push(' ');
        write_coord(&mut out, y);
    }
    out.push_str("\" fill=\"#000000\"/></g></g></svg>\n");
    out
}

/// `value` redondeado a tres decimales (mitad hacia arriba) y escrito en
/// `out` con [`crate::format::float_to_string`].
fn write_coord(out: &mut String, value: f64) {
    format::write_float(out, format::round_half_up(value, 3));
}

#[derive(Debug, Clone, Copy)]
enum PathOp {
    Move(f64, f64),
    Line(f64, f64),
    Close,
}

#[allow(clippy::float_arithmetic, reason = "traslación y giro de trazados")]
impl PathOp {
    fn translate(self, dx: f64, dy: f64) -> Self {
        match self {
            Self::Move(x, y) => Self::Move(x + dx, y + dy),
            Self::Line(x, y) => Self::Line(x + dx, y + dy),
            Self::Close => Self::Close,
        }
    }

    fn rotate(self, degrees: i32) -> Self {
        let radians = (f64::from(degrees) / 180.0) * std::f64::consts::PI;
        let (sin, cos) = (radians.sin(), radians.cos());
        let turn = |x: f64, y: f64| (x * cos - y * sin, x * sin + y * cos);
        match self {
            Self::Move(x, y) => {
                let (a, b) = turn(x, y);
                Self::Move(a, b)
            }
            Self::Line(x, y) => {
                let (a, b) = turn(x, y);
                Self::Line(a, b)
            }
            Self::Close => Self::Close,
        }
    }
}

/// Borra los tres patrones de posición (7×7), que se dibujan como ojos.
fn remove_position_patterns(bytes: &mut [u8], size: usize) {
    for (ox, oy) in [
        (0, 0),
        (size.saturating_sub(7), 0),
        (0, size.saturating_sub(7)),
    ] {
        for y in 0..7 {
            for x in 0..7 {
                if let Some(cell) = bytes.get_mut((oy + y) * size + ox + x) {
                    *cell = 0;
                }
            }
        }
    }
}

/// Contornos de las regiones de módulos a 1, como trazados cerrados
/// (algoritmo de seguimiento de bordes con relleno XOR, como potrace).
fn module_path(bytes: Vec<u8>, width: usize, height: usize) -> Vec<PathOp> {
    let mut tracer = EdgeTracer {
        bytes,
        width,
        height,
    };
    let mut ops = Vec::new();
    let mut start = Some((0usize, 0usize));
    while let Some(point) = start.and_then(|(x, y)| tracer.find_next(x, y)) {
        let points = tracer.find_edge(point.0, point.1);
        tracer.xor_edge(&points);
        let simplified = simplify(&points);
        if let Some(((x0, y0), rest)) = simplified.split_first() {
            ops.push(PathOp::Move(to_f64(*x0), to_f64(*y0)));
            for (x, y) in rest {
                ops.push(PathOp::Line(to_f64(*x), to_f64(*y)));
            }
            ops.push(PathOp::Close);
        }
        start = Some(point);
    }
    ops
}

fn to_f64(value: i64) -> f64 {
    // Coordenadas de módulos: |v| ≤ 177, exactas en f64.
    f64::from(i32::try_from(value).unwrap_or(0))
}

struct EdgeTracer {
    bytes: Vec<u8>,
    width: usize,
    height: usize,
}

impl EdgeTracer {
    fn find_next(&self, x: usize, y: usize) -> Option<(usize, usize)> {
        let start = self.width * y + x;
        let index = (start..self.bytes.len()).find(|i| self.bytes[*i] == 1)?;
        Some((index % self.width, index / self.width))
    }

    fn is_set(&self, x: i64, y: i64) -> bool {
        let (Ok(xu), Ok(yu)) = (usize::try_from(x), usize::try_from(y)) else {
            return false;
        };
        xu < self.width && yu < self.height && self.bytes[self.width * yu + xu] == 1
    }

    fn find_edge(&self, x: usize, y: usize) -> Vec<(i64, i64)> {
        let (start_x, start_y) = (to_i64(x), to_i64(y));
        let (mut x, mut y) = (start_x, start_y);
        let (mut dx, mut dy) = (0i64, 1i64);
        let mut points = Vec::new();
        loop {
            points.push((x, y));
            x += dx;
            y += dy;
            if x == start_x && y == start_y {
                break;
            }
            let left = self.is_set(x + (dx + dy - 1) / 2, y + (dy - dx - 1) / 2);
            let right = self.is_set(x + (dx - dy - 1) / 2, y + (dy + dx - 1) / 2);
            if right {
                (dx, dy) = (-dy, dx);
            } else if !left {
                (dx, dy) = (dy, -dx);
            }
        }
        points
    }

    fn xor_edge(&mut self, points: &[(i64, i64)]) {
        let Some(&(_, first_y)) = points.first() else {
            return;
        };
        let max_x = points.iter().map(|p| p.0).max().unwrap_or(-1);
        let mut y1 = first_y;
        for &(x, y) in &points[1..] {
            if y == y1 {
                continue;
            }
            self.flip_row(x, max_x, y1.min(y));
            y1 = y;
        }
    }

    /// Invierte los módulos `from..to` de la fila `y`. Dentro de la matriz
    /// se hace sobre el segmento contiguo; los módulos valen 0 o 1.
    fn flip_row(&mut self, from: i64, to: i64, y: i64) {
        let inside = |v: i64, limit: usize| usize::try_from(v).ok().filter(|v| *v <= limit);
        if let (Some(from), Some(to), Some(y)) = (
            inside(from, self.width),
            inside(to, self.width),
            inside(y, self.height.saturating_sub(1)),
        ) {
            if from <= to && y < self.height {
                let row = self.width * y;
                if let Some(cells) = self.bytes.get_mut(row + from..row + to) {
                    for cell in cells {
                        *cell ^= 1;
                    }
                    return;
                }
            }
        }
        for j in from..to {
            self.flip(j, y);
        }
    }

    fn flip(&mut self, x: i64, y: i64) {
        let set = self.is_set(x, y);
        if let (Ok(xu), Ok(yu)) = (usize::try_from(x), usize::try_from(y)) {
            if let Some(cell) = self.bytes.get_mut(self.width * yu + xu) {
                *cell = u8::from(!set);
            }
        }
    }
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Quita los puntos intermedios de cada tramo recto del contorno.
fn simplify(points: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let len = points.len();
    let mut out = Vec::new();
    for i in 0..len {
        let prev = points[if i == 0 { len - 1 } else { i - 1 }];
        let next = points[if i == len - 1 { 0 } else { i + 1 }];
        let cur = points[i];
        if (prev.0 == cur.0 && cur.0 == next.0) || (prev.1 == cur.1 && cur.1 == next.1) {
            continue;
        }
        out.push(cur);
    }
    out
}

/// Trazados de un ojo centrados en el origen:
/// anillo exterior 7×7 y cuadro interior 3×3. No dependen del contenido.
static EYE_PATHS: LazyLock<[Vec<PathOp>; 2]> = LazyLock::new(|| {
    let mut ring = vec![0u8; 49];
    for i in 0..7 {
        ring[i] = 1;
        ring[6 * 7 + i] = 1;
        ring[i * 7] = 1;
        ring[i * 7 + 6] = 1;
    }
    let centered = |ops: Vec<PathOp>, half: f64| -> Vec<PathOp> {
        ops.into_iter()
            .map(|op| op.translate(-half, -half))
            .collect()
    };
    [
        centered(module_path(ring, 7, 7), 3.5),
        centered(module_path(vec![1u8; 9], 3, 3), 1.5),
    ]
});

/// Los tres ojos: arriba a la izquierda, arriba a la derecha (girado 90°) y
/// abajo a la izquierda (girado −90°).
#[allow(clippy::float_arithmetic, reason = "posiciones de los ojos")]
fn append_eyes(ops: &mut Vec<PathOp>, size: usize) {
    let far = to_f64(to_i64(size)) - 3.5;
    for (x, y, rotation) in [(3.5, 3.5, 0), (far, 3.5, 90), (3.5, far, -90)] {
        for path in EYE_PATHS.iter() {
            ops.extend(path.iter().map(|op| op.rotate(rotation).translate(x, y)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_follow_bacon() {
        assert_eq!(choose_mode(b"123"), QrMode::Numeric);
        assert_eq!(choose_mode(b"HTTPS://X.PE/A"), QrMode::Alphanumeric);
        assert_eq!(choose_mode(b"20123|01|"), QrMode::Byte);
        assert_eq!(encode(b""), Err(QrError::EmptyContent));
        assert_eq!(encode(&[0xff, 0xfe]), Err(QrError::InvalidUtf8));
        assert_eq!(encode(&[b'x'; 1664]), Err(QrError::DataTooBig));
    }

    #[test]
    fn capacity_table_matches_qrcodegen() {
        for version in 1..=40u8 {
            let capacity = DATA_CODEWORDS_Q[usize::from(version - 1)] * 8;
            let header = 4 + count_bits(QrMode::Byte, version);
            let fits = usize::try_from((capacity - header) / 8).unwrap();
            let v = Version::new(version);
            let ok = QrCode::encode_segments_advanced(
                &[QrSegment::make_bytes(&vec![b'x'; fits])],
                QrCodeEcc::Quartile,
                v,
                v,
                None,
                false,
            );
            assert!(ok.is_ok(), "versión {version}");
            let too_big = QrCode::encode_segments_advanced(
                &[QrSegment::make_bytes(&vec![b'x'; fits + 1])],
                QrCodeEcc::Quartile,
                v,
                v,
                None,
                false,
            );
            assert!(too_big.is_err(), "versión {version}");
        }
    }

    #[test]
    fn version_selection_requires_an_extra_byte() {
        // 27 dígitos: 104 bits caben justos en v1-Q (13 bytes); se elige v2.
        assert_eq!(select_version(QrMode::Numeric, 27), Some(2));
        assert_eq!(select_version(QrMode::Byte, 1661), Some(40));
        assert_eq!(select_version(QrMode::Byte, 1662), None);
    }

    /// Reglas N1–N4 escritas directamente sobre filas anidadas, sin segmentos
    /// contiguos ni conteo por ventanas: referencia de equivalencia.
    #[allow(clippy::needless_range_loop, reason = "referencia literal")]
    fn reference_penalty(m: &[Vec<bool>]) -> u32 {
        let n = m.len();
        let mut p1 = 0u32;
        for horizontal in [true, false] {
            for i in 0..n {
                let (mut same, mut prev) = (0u32, None);
                for j in 0..n {
                    let bit = if horizontal { m[i][j] } else { m[j][i] };
                    if prev == Some(bit) {
                        same += 1;
                    } else {
                        if same >= 5 {
                            p1 += 3 + (same - 5);
                        }
                        same = 1;
                        prev = Some(bit);
                    }
                }
                if same >= 5 {
                    p1 += 3 + (same - 5);
                }
            }
        }
        let mut p2 = 0u32;
        for y in 0..n - 1 {
            for x in 0..n - 1 {
                let v = m[y][x];
                if v == m[y][x + 1] && v == m[y + 1][x] && v == m[y + 1][x + 1] {
                    p2 += 3;
                }
            }
        }
        const PATTERN: [bool; 7] = [true, false, true, true, true, false, true];
        let mut p3 = 0u32;
        for y in 0..n {
            for x in 0..n {
                if x + 6 < n && (0..7).all(|k| m[y][x + k] == PATTERN[k]) {
                    let light = |a: usize, b: usize| (a..b).all(|k| !m[y][k]);
                    if (x + 10 < n && light(x + 7, x + 11)) || (x >= 4 && light(x - 4, x)) {
                        p3 += 40;
                    }
                }
                if y + 6 < n && (0..7).all(|k| m[y + k][x] == PATTERN[k]) {
                    let light = |a: usize, b: usize| (a..b).all(|k| !m[k][x]);
                    if (y + 10 < n && light(y + 7, y + 11)) || (y >= 4 && light(y - 4, y)) {
                        p3 += 40;
                    }
                }
            }
        }
        let grid = Grid {
            size: n,
            cells: m.concat(),
            transposed: Vec::new(),
        };
        p1 + p2 + p3 + rule4(&grid)
    }

    #[test]
    fn mask_penalty_equals_reference() {
        let mut state = 0x0123_4567_89AB_CDEFu64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let alphabet: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:abcxyz|";
        for _ in 0..120 {
            let len = usize::try_from(next() % 600 + 1).unwrap();
            let pool = match next() % 3 {
                0 => &alphabet[..10],
                1 => &alphabet[..45],
                _ => alphabet,
            };
            let content: Vec<u8> = (0..len)
                .map(|_| pool[usize::try_from(next() % pool.len() as u64).unwrap()])
                .collect();
            let text = std::str::from_utf8(&content).unwrap();
            let segments = match choose_mode(&content) {
                QrMode::Numeric => vec![QrSegment::make_numeric(text)],
                QrMode::Alphanumeric => vec![QrSegment::make_alphanumeric(text)],
                QrMode::Byte => vec![
                    QrSegment::make_eci(ECI_UTF8),
                    QrSegment::make_bytes(&content),
                ],
            };
            for mask in 0..8u8 {
                let code = QrCode::encode_segments_advanced(
                    &segments,
                    QrCodeEcc::Quartile,
                    Version::MIN,
                    Version::MAX,
                    Some(Mask::new(mask)),
                    false,
                )
                .unwrap();
                let grid = Grid::of(&code);
                assert_eq!(mask_penalty(&grid), reference_penalty(&grid.rows()));
            }
        }
    }

    #[test]
    fn mask_deltas_reproduce_every_mask_in_every_version() {
        let mut state = 0xA076_1D64_78BD_642Fu64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for version in 1..=40u8 {
            let v = Version::new(version);
            let capacity = DATA_CODEWORDS_Q[usize::from(version - 1)] * 8;
            let fits =
                usize::try_from((capacity - 4 - 8 - 4 - count_bits(QrMode::Byte, version)) / 8)
                    .unwrap();
            let deltas = mask_deltas(v).unwrap();
            for round in 0..3 {
                let len = if round == 0 {
                    fits
                } else {
                    usize::try_from(next()).unwrap() % fits + 1
                };
                let content: Vec<u8> = (0..len).map(|_| next().to_le_bytes()[0]).collect();
                let segments = [
                    QrSegment::make_eci(ECI_UTF8),
                    QrSegment::make_bytes(&content),
                ];
                let encode = |mask: u8| {
                    Grid::of(
                        &QrCode::encode_segments_advanced(
                            &segments,
                            QrCodeEcc::Quartile,
                            v,
                            v,
                            Some(Mask::new(mask)),
                            false,
                        )
                        .unwrap(),
                    )
                };
                let base = encode(0);
                for mask in 1..8u8 {
                    let derived = base.xor(&deltas.words[usize::from(mask - 1)]);
                    let direct = encode(mask);
                    assert!(
                        derived.cells == direct.cells && derived.transposed == direct.transposed,
                        "versión {version}, máscara {mask}"
                    );
                }
            }
        }
    }

    #[test]
    fn known_symbol() {
        // Del corpus: la variante ZXing elige la máscara 2; la puntuación
        // propia de qrcodegen elegiría la 0.
        let symbol =
            encode(b"20123456789|01|F001|123|18.00|118.00|2024-03-15|6|20000000002|").unwrap();
        assert_eq!((symbol.version, symbol.mask), (6, 2));
    }
}
