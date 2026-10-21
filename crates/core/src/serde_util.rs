//! Adaptadores serde internos.

use std::fmt;

/// Serializa un valor como texto con su `Display`, sin crear un `String`
/// intermedio (`collect_str`).
struct AsText<'a, T: ?Sized>(&'a T);

impl<T: fmt::Display + ?Sized> serde::Serialize for AsText<'_, T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self.0)
    }
}

/// Longitud máxima de la entrada que se repite en un mensaje de error: un
/// texto de varios megabytes no debe copiarse entero en el error ni en logs.
const PREVIEW_CHARS: usize = 64;

/// La entrada entre comillas, recortada a [`PREVIEW_CHARS`] caracteres.
fn preview(text: &str) -> String {
    match text.char_indices().nth(PREVIEW_CHARS) {
        Some((end, _)) => format!("{:?}… ({} bytes)", &text[..end], text.len()),
        None => format!("{text:?}"),
    }
}

/// `Option<Vec<u8>>` como texto Base64 estándar (contenido binario como el ZIP del CDR).
pub(crate) mod base64_option {
    use std::fmt;

    use base64::Engine as _;
    use base64::display::Base64Display;
    use base64::engine::general_purpose::STANDARD;
    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};

    use super::AsText;

    #[allow(clippy::ref_option)]
    pub(crate) fn serialize<S: Serializer>(
        value: &Option<Vec<u8>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(bytes) => {
                serializer.serialize_some(&AsText(&Base64Display::new(bytes, &STANDARD)))
            }
            None => serializer.serialize_none(),
        }
    }

    /// Decodifica el texto prestado por el formato, sin copiarlo antes en un
    /// `String` (un CDR de 1 MiB son 1,3 MB de Base64). Acepta y rechaza lo
    /// mismo que `Option<String>`, con los mismos mensajes.
    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Vec<u8>>, D::Error> {
        deserializer.deserialize_option(OptionVisitor)
    }

    struct OptionVisitor;

    impl<'de> Visitor<'de> for OptionVisitor {
        type Value = Option<Vec<u8>>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("option")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_string(TextVisitor).map(Some)
        }
    }

    /// Texto Base64, con el `expecting` de `String`.
    struct TextVisitor;

    impl Visitor<'_> for TextVisitor {
        type Value = Vec<u8>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string")
        }

        fn visit_str<E: de::Error>(self, text: &str) -> Result<Vec<u8>, E> {
            STANDARD.decode(text).map_err(E::custom)
        }

        fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Vec<u8>, E> {
            match std::str::from_utf8(bytes) {
                Ok(text) => self.visit_str(text),
                Err(_) => Err(E::invalid_value(de::Unexpected::Bytes(bytes), &self)),
            }
        }
    }
}

/// `Option<Decimal>` serializado como texto exacto (`"18.50"`).
///
/// Al deserializar acepta:
/// - texto decimal (`"-15.30"`) o en notación científica (`"1e3"`, `"1.0e-6"`),
///   sin redondeo silencioso: un valor que no cabe en `Decimal` es un error;
/// - números JSON enteros, que se convierten sin pérdida.
///
/// Rechaza números JSON con decimales porque ya llegan como `f64` y podrían
/// haber perdido precisión, y textos con `_` (`rust_decimal` lo admitiría
/// como separador: `"1_00"` valdría `100`). Conserva el signo de
/// `-0`/`-0.00`, como el `float` de la referencia.
pub(crate) mod decimal_option {
    use std::fmt;

    use rust_decimal::Decimal;
    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};

    use super::{AsText, preview};

    #[allow(clippy::ref_option)]
    pub(crate) fn serialize<S: Serializer>(
        value: &Option<Decimal>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(decimal) => serializer.serialize_some(&AsText(decimal)),
            None => serializer.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Decimal>, D::Error> {
        deserializer.deserialize_option(OptionVisitor)
    }

    /// Interpreta un texto decimal exacto o en notación científica.
    pub(crate) fn parse(text: &str) -> Result<Decimal, String> {
        let invalid = |err: &dyn fmt::Display| format!("importe inválido {}: {err}", preview(text));
        if text.contains('_') {
            return Err(invalid(&"separador '_' no admitido"));
        }
        let mut decimal = if text.contains(['e', 'E']) {
            Decimal::from_scientific(text).map_err(|err| invalid(&err))?
        } else {
            Decimal::from_str_exact(text).map_err(|err| invalid(&err))?
        };
        if decimal.is_zero() && text.trim_start().starts_with('-') {
            decimal.set_sign_negative(true);
        }
        Ok(decimal)
    }

    struct OptionVisitor;

    impl<'de> Visitor<'de> for OptionVisitor {
        type Value = Option<Decimal>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("un importe decimal como texto, un entero o null")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_any(ValueVisitor).map(Some)
        }
    }

    struct ValueVisitor;

    impl Visitor<'_> for ValueVisitor {
        type Value = Decimal;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("un importe decimal como texto o un entero")
        }

        fn visit_str<E: de::Error>(self, text: &str) -> Result<Decimal, E> {
            parse(text).map_err(E::custom)
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<Decimal, E> {
            Ok(Decimal::from(value))
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Decimal, E> {
            Ok(Decimal::from(value))
        }

        fn visit_f64<E: de::Error>(self, value: f64) -> Result<Decimal, E> {
            Err(E::custom(format!(
                "importe {value} recibido como número JSON con decimales; enviarlo como texto para no perder precisión"
            )))
        }
    }
}

#[cfg(test)]
mod decimal_option_tests {
    use serde::de::IntoDeserializer;
    use serde::de::value::{Error, F64Deserializer};

    use super::decimal_option::{deserialize, parse};

    /// Un importe nunca es `NaN` ni infinito: `Decimal` no los representa,
    /// los textos se rechazan y un `f64` (incluidos `NaN`/`inf`) también.
    #[test]
    fn non_finite_amounts_are_rejected() {
        for text in [
            "NaN",
            "nan",
            "-NaN",
            "inf",
            "-inf",
            "INF",
            "Infinity",
            "-Infinity",
            "1e400",
            "-1e400",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1.5] {
            let de: F64Deserializer<Error> = value.into_deserializer();
            assert!(deserialize(de).is_err(), "{value}");
        }
    }
}

/// `Option<Zoned>` como texto RFC 9557 (`2024-01-31T23:30:00-05:00[America/Lima]`).
///
/// Al deserializar acepta además RFC 3339 con desfase y sin zona
/// (`2024-01-31T23:30:00-05:00`, `...Z`), el formato habitual de un
/// `DateTime` de la referencia serializado; en ese caso la zona es el desfase fijo, como la
/// zona de tipo «offset» de la referencia.
pub(crate) mod zoned_option {
    use std::str::FromStr;

    use jiff::tz::{Offset, TimeZone};
    use jiff::{Timestamp, Zoned};
    use serde::{Deserialize, Deserializer, Serializer};

    use super::{AsText, preview};

    #[allow(clippy::ref_option)]
    pub(crate) fn serialize<S: Serializer>(
        value: &Option<Zoned>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(zoned) => serializer.serialize_some(&AsText(zoned)),
            None => serializer.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Zoned>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|text| parse(&text).map_err(serde::de::Error::custom))
            .transpose()
    }

    fn parse(text: &str) -> Result<Zoned, String> {
        let invalid = |err: jiff::Error| format!("fecha inválida {}: {err}", preview(text));
        if text.contains('[') {
            return Zoned::from_str(text).map_err(invalid);
        }
        let timestamp = Timestamp::from_str(text).map_err(invalid)?;
        let offset = trailing_offset(text)
            .ok_or_else(|| format!("fecha sin desfase ±HH:MM ni Z: {}", preview(text)))?;
        Ok(timestamp.to_zoned(TimeZone::fixed(offset)))
    }

    /// Desfase final `Z` o `±HH:MM` de un texto RFC 3339.
    fn trailing_offset(text: &str) -> Option<Offset> {
        if text.ends_with(['Z', 'z']) {
            return Some(Offset::UTC);
        }
        let suffix = text.get(text.len().checked_sub(6)?..)?;
        let (sign, rest) = match suffix.as_bytes().first()? {
            b'+' => (1, &suffix[1..]),
            b'-' => (-1, &suffix[1..]),
            _ => return None,
        };
        let (hours, minutes) = rest.split_once(':')?;
        let seconds = hours.parse::<i32>().ok()? * 3600 + minutes.parse::<i32>().ok()? * 60;
        Offset::from_seconds(sign * seconds).ok()
    }

    #[cfg(test)]
    mod tests {
        use super::parse;

        #[test]
        fn accepts_rfc9557_and_rfc3339() {
            let named = parse("2024-01-31T23:30:00-05:00[America/Lima]").unwrap();
            assert_eq!(named.time_zone().iana_name(), Some("America/Lima"));
            let fixed = parse("2024-01-31T23:30:00-05:00").unwrap();
            assert_eq!(fixed.offset().seconds(), -18_000);
            assert_eq!(fixed.timestamp(), named.timestamp());
            assert_eq!(
                parse("2024-02-01T04:30:00Z").unwrap().timestamp(),
                named.timestamp()
            );
            assert!(parse("2024-01-01").is_err());
            assert!(parse("2024-01-01T00:00:00").is_err());
        }
    }
}
