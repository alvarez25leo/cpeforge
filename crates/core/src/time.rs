//! Fechas con la zona horaria de Perú.
//!
//! La base de zonas horarias va incluida en el binario (`jiff-tzdb`) para que
//! el resultado no dependa del sistema operativo.

use std::sync::OnceLock;

use jiff::Zoned;
use jiff::civil::Date;
use jiff::tz::TimeZone;

use crate::CoreError;

/// Identificador IANA de la zona horaria de Perú (`TimeZonePe::DEFAULT`).
pub const PERU_TIME_ZONE: &str = "America/Lima";

/// Zona horaria `America/Lima`, incluidas sus reglas históricas
/// (hora media local antes de 1908 y horarios de verano de 1986–1994).
///
/// # Errors
///
/// [`CoreError::TimeZone`] si la base tz incluida no contiene la zona.
pub fn peru_time_zone() -> Result<TimeZone, CoreError> {
    // La zona es inmutable: se busca una vez y después solo se clona (un
    // contador de referencias). Un fallo no se guarda.
    static PERU: OnceLock<TimeZone> = OnceLock::new();
    if let Some(tz) = PERU.get() {
        return Ok(tz.clone());
    }
    let tz = TimeZone::get(PERU_TIME_ZONE).map_err(|err| CoreError::TimeZone {
        message: err.to_string(),
    })?;
    Ok(PERU.get_or_init(|| tz).clone())
}

/// El mismo instante expresado en la zona horaria de Perú.
///
/// No modifica la fecha recibida.
///
/// # Errors
///
/// [`CoreError::TimeZone`] si la zona de Perú no está disponible.
pub fn to_peru_time(date: &Zoned) -> Result<Zoned, CoreError> {
    Ok(date.with_time_zone(peru_time_zone()?))
}

/// Fecha civil `AAAAMMDD`: año con al menos cuatro cifras (y signo `-` para
/// años negativos), mes y día con dos cifras.
pub(crate) fn format_ymd(date: Date) -> String {
    let year = i32::from(date.year());
    let sign = if year < 0 { "-" } else { "" };
    format!(
        "{sign}{:04}{:02}{:02}",
        year.unsigned_abs(),
        date.month(),
        date.day()
    )
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    #[test]
    fn format_ymd_pads_with_zeros() {
        assert_eq!(format_ymd(date(2024, 2, 29)), "20240229");
        assert_eq!(format_ymd(date(999, 6, 15)), "09990615");
        assert_eq!(format_ymd(date(0, 1, 1)), "00000101");
        assert_eq!(format_ymd(date(-1, 12, 31)), "-00011231");
        assert_eq!(format_ymd(date(9999, 12, 31)), "99991231");
    }

    #[test]
    fn peru_zone_is_available_and_named() {
        let tz = peru_time_zone().unwrap();
        assert_eq!(tz.iana_name(), Some(PERU_TIME_ZONE));
    }
}
