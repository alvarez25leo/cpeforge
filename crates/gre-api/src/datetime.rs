//! Fechas `date-time` de las respuestas (`fecRecepcion`).
//!
//! El texto recibido se conserva en [`DateTimeValue`] y solo se interpreta
//! como ISO 8601 / RFC 3339 **con desplazamiento explícito**. Las fechas
//! relativas, los formatos libres y los textos sin desplazamiento se informan
//! como no interpretables en lugar de suponer una zona horaria.
//!
//! Un texto sin forma de fecha (`AAAA-MM-DD…`) falla ya al deserializar la
//! respuesta, con el error `DateMalformedStringException`. Las fechas sin
//! desplazamiento o imposibles se conservan y [`DateTimeValue::parse`] informa
//! el motivo.

use std::fmt;

/// Texto `date-time` recibido y su interpretación.
#[derive(Clone, PartialEq, Eq)]
pub struct DateTimeValue {
    raw: String,
}

/// Instante con desplazamiento, con precisión de microsegundos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParsedDateTime {
    /// Año.
    pub year: i32,
    /// Mes (1-12).
    pub month: u8,
    /// Día (1-31).
    pub day: u8,
    /// Hora (0-23).
    pub hour: u8,
    /// Minuto.
    pub minute: u8,
    /// Segundo.
    pub second: u8,
    /// Microsegundos (los dígitos adicionales se truncan, no se redondean).
    pub microsecond: u32,
    /// Desplazamiento respecto de UTC en segundos.
    pub offset_seconds: i32,
}

/// Por qué no se pudo interpretar una fecha.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DateTimeError {
    /// No tiene la forma `AAAA-MM-DD[THH:MM:SS[.f]](Z|±HH:MM|±HHMM)`.
    Format,
    /// No indica desplazamiento: interpretarla exigiría suponer una zona
    /// horaria.
    MissingOffset,
    /// Componentes fuera de rango.
    Range,
}

impl fmt::Display for DateTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Format => "fecha con formato no admitido",
            Self::MissingOffset => "fecha sin desplazamiento horario",
            Self::Range => "fecha fuera de rango",
        })
    }
}

impl std::error::Error for DateTimeError {}

impl DateTimeValue {
    /// Envuelve el texto recibido.
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self { raw: raw.into() }
    }

    /// Texto tal como llegó.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Interpreta el texto (ver el módulo).
    ///
    /// # Errors
    ///
    /// [`DateTimeError`] si no es ISO 8601 con desplazamiento.
    pub fn parse(&self) -> Result<ParsedDateTime, DateTimeError> {
        parse(&self.raw)
    }
}

impl fmt::Debug for DateTimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DateTimeValue(<oculto>)")
    }
}

impl ParsedDateTime {
    /// `AAAA-MM-DDTHH:MM:SS.ffffff±HH:MM`.
    #[must_use]
    pub fn format_micro(&self) -> String {
        format!("{}.{:06}{}", self.base(), self.microsecond, self.offset())
    }

    /// `AAAA-MM-DDTHH:MM:SS±HH:MM` (sin fracción), el formato con que se
    /// serializa `fecRecepcion`.
    #[must_use]
    pub fn format_atom(&self) -> String {
        format!("{}{}", self.base(), self.offset())
    }

    fn base(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    fn offset(&self) -> String {
        let sign = if self.offset_seconds < 0 { '-' } else { '+' };
        let abs = self.offset_seconds.unsigned_abs();
        format!("{sign}{:02}:{:02}", abs / 3600, abs % 3600 / 60)
    }
}

fn digits(s: &str, n: usize) -> Option<u32> {
    if s.len() == n && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}

fn parse(text: &str) -> Result<ParsedDateTime, DateTimeError> {
    let t = text.trim();
    if t.len() < 10 {
        return Err(DateTimeError::Format);
    }
    let date = t.get(..10).ok_or(DateTimeError::Format)?;
    let rest = t.get(10..).ok_or(DateTimeError::Format)?;
    let mut parts = date.split('-');
    let (Some(y), Some(mo), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(DateTimeError::Format);
    };
    let year = digits(y, 4).ok_or(DateTimeError::Format)?;
    let month = digits(mo, 2).ok_or(DateTimeError::Format)?;
    let day = digits(d, 2).ok_or(DateTimeError::Format)?;
    let (mut hour, mut minute, mut second, mut micro) = (0, 0, 0, 0);
    let mut rest = rest;
    if let Some(time) = rest.strip_prefix('T').or_else(|| rest.strip_prefix(' ')) {
        if time.len() < 8 {
            return Err(DateTimeError::Format);
        }
        let hms = time.get(..8).ok_or(DateTimeError::Format)?;
        let after = time.get(8..).ok_or(DateTimeError::Format)?;
        let mut p = hms.split(':');
        let (Some(h), Some(mi), Some(s), None) = (p.next(), p.next(), p.next(), p.next()) else {
            return Err(DateTimeError::Format);
        };
        hour = digits(h, 2).ok_or(DateTimeError::Format)?;
        minute = digits(mi, 2).ok_or(DateTimeError::Format)?;
        second = digits(s, 2).ok_or(DateTimeError::Format)?;
        rest = after;
        if let Some(frac) = rest.strip_prefix('.') {
            let n = frac.bytes().take_while(u8::is_ascii_digit).count();
            if n == 0 {
                return Err(DateTimeError::Format);
            }
            let mut f6: String = frac[..n].chars().take(6).collect();
            while f6.len() < 6 {
                f6.push('0');
            }
            micro = f6.parse().map_err(|_| DateTimeError::Format)?;
            rest = &frac[n..];
        }
    }
    let offset_seconds = match rest {
        "" => return Err(DateTimeError::MissingOffset),
        "Z" | "z" => 0,
        o => {
            let sign = match o.as_bytes().first() {
                Some(b'+') => 1,
                Some(b'-') => -1,
                _ => return Err(DateTimeError::Format),
            };
            let body = o.get(1..).ok_or(DateTimeError::Format)?;
            let (h, m) = match body.len() {
                5 if body.as_bytes()[2] == b':' => (
                    body.get(..2).ok_or(DateTimeError::Format)?,
                    body.get(3..).ok_or(DateTimeError::Format)?,
                ),
                4 => (
                    body.get(..2).ok_or(DateTimeError::Format)?,
                    body.get(2..).ok_or(DateTimeError::Format)?,
                ),
                _ => return Err(DateTimeError::Format),
            };
            let h = digits(h, 2).ok_or(DateTimeError::Format)?;
            let m = digits(m, 2).ok_or(DateTimeError::Format)?;
            sign * i32::try_from(h * 3600 + m * 60).map_err(|_| DateTimeError::Range)?
        }
    };
    if !(1..=12).contains(&month)
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return Err(DateTimeError::Range);
    }
    let small = |v: u32| u8::try_from(v).map_err(|_| DateTimeError::Range);
    Ok(ParsedDateTime {
        year: i32::try_from(year).map_err(|_| DateTimeError::Range)?,
        month: small(month)?,
        day: small(day)?,
        hour: small(hour)?,
        minute: small(minute)?,
        second: small(second)?,
        microsecond: micro,
        offset_seconds,
    })
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_with_offset() {
        let f = |s: &str| DateTimeValue::new(s).parse().map(|p| p.format_micro());
        assert_eq!(
            f("2023-01-02T10:20:30-05:00").unwrap(),
            "2023-01-02T10:20:30.000000-05:00"
        );
        assert_eq!(
            f("2023-01-02T10:20:30.1234567-05:00").unwrap(),
            "2023-01-02T10:20:30.123456-05:00"
        );
        assert_eq!(
            f("2023-01-02T10:20:30Z").unwrap(),
            "2023-01-02T10:20:30.000000+00:00"
        );
        assert_eq!(
            f("2023-01-02T10:20:30+0500").unwrap(),
            "2023-01-02T10:20:30.000000+05:00"
        );
        assert_eq!(f("2023-01-02T10:20:30"), Err(DateTimeError::MissingOffset));
        assert_eq!(f("tomorrow"), Err(DateTimeError::Format));
        assert_eq!(f("2023-02-30T00:00:00Z"), Err(DateTimeError::Range));
        assert_eq!(f("ññññññññññ"), Err(DateTimeError::Format));
        assert_eq!(f("2024-01-01Tñññññ"), Err(DateTimeError::Format));
    }
}
