//! Hidratación de casos neutrales en modelos de `cpeforge-core`.
//!
//! Compartido por `crates/core/tests/parity.rs` y `crates/xml/tests/parity.rs`
//! (incluido con `#[path]`): convierte la gramática de los fixtures
//! (`$class`, `$ctor`, `$decimal`, `$datetime`, `$bytes`) en el JSON serde de
//! los modelos llamando a los constructores Rust.

use std::error::Error as StdError;
use std::fmt;
use std::str::FromStr;

use cpeforge_core::Decimal;
use cpeforge_core::Zoned;
use cpeforge_core::model::despatch::Direction;
use cpeforge_core::model::response::ResponseError;
use cpeforge_core::model::sale::PaymentTerms;
use cpeforge_core::service::BasicToken;
use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

/// Fallo al construir el objeto raíz del caso.
#[derive(Debug)]
pub(crate) enum HydrateError {
    /// serde rechazó un valor; `path` es la ruta del campo.
    Serde { path: String, message: String },
    /// Constructor llamado con menos argumentos de los obligatorios: en Rust
    /// no compila, así que el arnés lo informa sin llamar.
    Arity {
        class: String,
        given: usize,
        expected: usize,
    },
    /// El valor de entrada no cabe en el tipo Rust (p. ej. fecha fuera del rango de `jiff`).
    Unrepresentable { message: String },
}

impl fmt::Display for HydrateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serde { path, message } => write!(f, "serde en `{path}`: {message}"),
            Self::Arity {
                class,
                given,
                expected,
            } => write!(f, "{class}: {given} argumentos, se requieren {expected}"),
            Self::Unrepresentable { message } => write!(f, "valor no representable: {message}"),
        }
    }
}

pub(crate) fn harness_error(message: impl Into<String>) -> Box<dyn StdError> {
    message.into().into()
}

/// Convierte un nodo de caso en el JSON serde equivalente.
pub(crate) fn to_serde(node: &Value) -> TestResult<Result<Value, HydrateError>> {
    match node {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match to_serde(item)? {
                    Ok(value) => out.push(value),
                    Err(err) => return Ok(Err(err)),
                }
            }
            Ok(Ok(Value::Array(out)))
        }
        Value::Object(map) if map.contains_key("$bytes") => {
            // Base64 estándar, que es la representación serde de los campos binarios.
            Ok(Ok(map["$bytes"].clone()))
        }
        Value::Object(map) if map.contains_key("$decimal") => {
            // El texto se entrega tal cual: lo interpreta el adaptador serde del crate.
            Ok(Ok(map["$decimal"].clone()))
        }
        Value::Object(map) if map.contains_key("$datetime") => match datetime(map) {
            Ok(zoned) => Ok(Ok(serde_json::to_value(zoned)?)),
            Err(err) => Ok(Err(HydrateError::Unrepresentable {
                message: err.to_string(),
            })),
        },
        Value::Object(map) if map.contains_key("$class") => build_object(map),
        Value::Object(_) => Err(harness_error(format!("nodo objeto sin $class: {node}"))),
        scalar => Ok(Ok(scalar.clone())),
    }
}

/// Instante RFC 3339 trasladado a la zona indicada (misma semántica que el oráculo de referencia).
pub(crate) fn datetime(map: &Map<String, Value>) -> TestResult<Zoned> {
    let instant = map["$datetime"].as_str().ok_or("$datetime no es texto")?;
    let tz_name = map["tz"].as_str().ok_or("tz no es texto")?;
    let timestamp = Timestamp::from_str(instant)?;
    Ok(timestamp.to_zoned(time_zone(tz_name)?))
}

pub(crate) fn time_zone(name: &str) -> TestResult<TimeZone> {
    if let Some(rest) = name.strip_prefix(['+', '-']) {
        let (hours, minutes) = rest.split_once(':').ok_or("desfase sin ':'")?;
        let seconds = hours.parse::<i32>()? * 3600 + minutes.parse::<i32>()? * 60;
        let seconds = if name.starts_with('-') {
            -seconds
        } else {
            seconds
        };
        return Ok(TimeZone::fixed(Offset::from_seconds(seconds)?));
    }
    Ok(TimeZone::get(name)?)
}

pub(crate) fn ctor_arg<T: DeserializeOwned>(
    args: &[Value],
    index: usize,
) -> TestResult<Result<Option<T>, HydrateError>> {
    let Some(node) = args.get(index) else {
        return Ok(Ok(None));
    };
    let value = match to_serde(node)? {
        Ok(value) => value,
        Err(err) => return Ok(Err(err)),
    };
    Ok(deserialize::<Option<T>>(value).map_err(|err| match err {
        HydrateError::Serde { message, .. } => HydrateError::Serde {
            path: format!("$ctor[{index}]"),
            message,
        },
        other => other,
    }))
}

/// Ejecuta el constructor Rust equivalente y aplica después los setters del caso.
pub(crate) fn build_object(map: &Map<String, Value>) -> TestResult<Result<Value, HydrateError>> {
    let class = map["$class"].as_str().ok_or("$class no es texto")?;
    let args: &[Value] = match map.get("$ctor") {
        Some(Value::Array(args)) => args,
        Some(_) => return Err(harness_error("$ctor no es lista")),
        None => &[],
    };
    macro_rules! arg {
        ($ty:ty, $index:expr) => {
            match ctor_arg::<$ty>(args, $index)? {
                Ok(value) => value,
                Err(err) => return Ok(Err(err)),
            }
        };
    }
    let arity = |expected: usize| HydrateError::Arity {
        class: class.to_owned(),
        given: args.len(),
        expected,
    };
    let mut base = match class {
        "Direction" => {
            if args.len() < 2 {
                return Ok(Err(arity(2)));
            }
            serde_json::to_value(Direction::new(arg!(String, 0), arg!(String, 1)))?
        }
        "BasicToken" => {
            if args.len() < 2 {
                return Ok(Err(arity(2)));
            }
            serde_json::to_value(BasicToken::new(arg!(String, 0), arg!(Zoned, 1)))?
        }
        "FormaPagoContado" => serde_json::to_value(PaymentTerms::contado())?,
        "FormaPagoCredito" => {
            let monto = match args.first() {
                Some(node) => match to_serde(node)? {
                    Ok(Value::String(text)) => Some(Decimal::from_str(&text)?),
                    Ok(Value::Null) => None,
                    Ok(other) => return Err(harness_error(format!("monto inesperado {other}"))),
                    Err(err) => return Ok(Err(err)),
                },
                None => None,
            };
            serde_json::to_value(PaymentTerms::credito(monto, arg!(String, 1)))?
        }
        "Error" => match args.len() {
            0 => serde_json::to_value(ResponseError::default())?,
            1 => serde_json::to_value(ResponseError::with_code(arg!(String, 0)))?,
            _ => serde_json::to_value(ResponseError::new(arg!(String, 0), arg!(String, 1)))?,
        },
        _ if !args.is_empty() => {
            return Err(harness_error(format!(
                "{class} no tiene constructor con argumentos"
            )));
        }
        _ => Value::Object(Map::new()),
    };
    let object = base
        .as_object_mut()
        .ok_or("constructor no produjo un objeto")?;
    for (key, value) in map {
        if key == "$class" || key == "$ctor" {
            continue;
        }
        match to_serde(value)? {
            Ok(value) => {
                object.insert(key.clone(), value);
            }
            Err(err) => return Ok(Err(err)),
        }
    }
    Ok(Ok(base))
}

pub(crate) fn deserialize<T: DeserializeOwned>(value: Value) -> Result<T, HydrateError> {
    serde_path_to_error::deserialize(value).map_err(|err| HydrateError::Serde {
        path: err.path().to_string(),
        message: err.inner().to_string(),
    })
}
