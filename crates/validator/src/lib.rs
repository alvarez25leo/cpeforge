//! Reglas de la referencia aplicadas a modelos de [`cpeforge_core`].
//!
//! Las propiedades y el orden de las 33 tablas base y siete variantes UBL 2.1
//! proceden de la metadata ejecutada de Symfony Validator 6.4.48. Los errores
//! de construcción de modelos se mantienen separados de las violaciones.
//!
//! ```
//! use cpeforge_core::model::client::Client;
//! use cpeforge_validator::Validator;
//!
//! let client = Client::default();
//! let errors = Validator::new().validate_model(&client)?;
//! assert_eq!(errors[0].path, "tipoDoc");
//! # Ok::<(), cpeforge_validator::ValidatorError>(())
//! ```

mod metadata;
mod nested;
mod setter_types;
mod translator;

pub use metadata::{FieldRules, Metadata, Rule, loader_names};
pub use setter_types::{SetterTypeError, check_setter_types};
pub use translator::MessageTranslator;

use cpeforge_core::Document;
use cpeforge_core::ports::ErrorCodeProvider;
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

const CODE_NOT_BLANK: &str = "c1051bb4-d103-4f74-8988-acbcafc7fdc3";
const CODE_NOT_NULL: &str = "ad32d13f-c3d4-423b-909a-857b961eb720";
const CODE_TOO_LONG: &str = "d94b19cc-114f-4f44-9cc4-4138e80a87b9";
const CODE_TOO_SHORT: &str = "9ff3fdc4-b214-49db-8718-39c315e33d45";
const CODE_EXACT_LENGTH: &str = "4b6f5c76-22b4-409d-af16-fbe823ba9332";
const CODE_REGEX: &str = "de1e3db3-5ed4-4941-aae4-59f3667cc3a3";
const CODE_CHOICE: &str = "8e179f1b-97aa-4560-a02f-2a8b42e49df7";
const CODE_TYPE: &str = "ba785a8c-82cb-4283-967c-3cf342181b40";
const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 100_000;
const MAX_VIOLATIONS: usize = 10_000;
const MAX_INPUT_BYTES: usize = 1_000_000;

/// Fallo al serializar un modelo, leer las reglas o validar una entrada acotada.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ValidatorError {
    /// La tabla congelada no se puede interpretar.
    RuleTable(String),
    /// El modelo no se pudo convertir a la representación de campos de la referencia.
    Model(String),
    /// El proveedor de mensajes falló.
    Provider(String),
    /// La entrada rebasa un límite de profundidad, tamaño o violaciones.
    Limit(&'static str),
    /// Apareció una restricción o patrón no implementado.
    UnsupportedRule(String),
}

impl fmt::Debug for ValidatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuleTable(_) => formatter.write_str("RuleTable([REDACTED])"),
            Self::Model(_) => formatter.write_str("Model([REDACTED])"),
            Self::Provider(_) => formatter.write_str("Provider([REDACTED])"),
            Self::Limit(limit) => formatter.debug_tuple("Limit").field(limit).finish(),
            Self::UnsupportedRule(_) => formatter.write_str("UnsupportedRule([REDACTED])"),
        }
    }
}

impl fmt::Display for ValidatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuleTable(_) => formatter.write_str("tabla de reglas inválida"),
            Self::Model(_) => formatter.write_str("modelo inválido"),
            Self::Provider(_) => formatter.write_str("proveedor de mensajes falló"),
            Self::Limit(limit) => write!(formatter, "límite de validación excedido: {limit}"),
            Self::UnsupportedRule(_) => formatter.write_str("restricción no soportada"),
        }
    }
}

impl std::error::Error for ValidatorError {}

/// Una violación de Symfony, incluida su ruta y los datos necesarios para comparar el oráculo.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub struct Violation {
    /// Ruta con índices de listas (`details[1].unidad`).
    pub path: String,
    /// Mensaje ya traducido.
    pub message: String,
    /// Plantilla previa a la traducción.
    pub template: String,
    /// Código estable de Symfony, si corresponde.
    pub code: Option<String>,
    /// Parámetros de la plantilla.
    pub parameters: BTreeMap<String, String>,
    /// Valor rechazado; puede contener datos de negocio y se redacta en `Debug`.
    pub invalid: Value,
}

impl fmt::Debug for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Violation")
            .field("path", &self.path)
            .field("message", &"[REDACTED]")
            .field("template", &"[REDACTED]")
            .field("code", &self.code)
            .field("parameters", &"[REDACTED]")
            .field("invalid", &"[REDACTED]")
            .finish()
    }
}

impl Violation {
    /// Crea una violación añadida por una extensión local.
    pub fn custom(path: impl Into<String>, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            path: path.into(),
            template: message.clone(),
            message,
            code: None,
            parameters: BTreeMap::new(),
            invalid: Value::Null,
        }
    }
}

/// Extiende reglas de propiedad y añade validaciones de clase antes de las propiedades.
pub trait RuleExtension: Send + Sync {
    /// Modifica la metadata efectiva después de elegir el loader y antes de validar.
    /// Solo se invoca para clases con loader, como `onLoaded` de la referencia.
    fn extend_metadata(&self, _model_type: &str, _metadata: &mut Metadata) {}

    /// Se ejecuta solo si la clase tiene loader, igual que `onLoaded` de la referencia.
    fn validate(&self, model_type: &str, model: &Value) -> Vec<Violation>;
}

type ProviderFn = dyn Fn(&str) -> Result<Option<String>, ValidatorError> + Send + Sync;

/// Validador sin estado global; cambiar la versión afecta a la siguiente llamada.
pub struct Validator {
    version: Option<String>,
    provider: Option<Box<ProviderFn>>,
    extension: Option<Box<dyn RuleExtension>>,
}

impl fmt::Debug for Validator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Validator")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    /// Crea el validador base sin proveedor de mensajes.
    pub fn new() -> Self {
        Self {
            version: None,
            provider: None,
            extension: None,
        }
    }

    /// Selecciona UBL 2.1; cualquier otra cadena recae en los loaders base de la referencia.
    pub fn set_version(&mut self, version: Option<&str>) {
        self.version = version.map(str::to_owned);
    }

    /// Conecta un proveedor del contrato de `cpeforge-core`.
    pub fn set_provider<P>(&mut self, provider: P)
    where
        P: ErrorCodeProvider + Send + Sync + 'static,
        P::Error: fmt::Display,
    {
        self.provider = Some(Box::new(move |code| {
            provider
                .value(Some(code))
                .map_err(|error| ValidatorError::Provider(error.to_string()))
        }));
    }

    /// Añade una extensión de validación sin mutación global.
    pub fn set_extension<E: RuleExtension + 'static>(&mut self, extension: E) {
        self.extension = Some(Box::new(extension));
    }

    /// Traduce como `MessageTranslator`: proveedor, recurso `G001` y luego ID.
    /// Parámetros, dominio y locale no modifican la salida de referencia fijada.
    pub fn translate(
        &self,
        id: &str,
        _parameters: &BTreeMap<String, String>,
        _domain: Option<&str>,
        _locale: Option<&str>,
    ) -> Result<String, ValidatorError> {
        if let Some(provider) = &self.provider {
            if let Some(message) = provider(id)? {
                if !message.is_empty() && message != "0" {
                    return Ok(message);
                }
            }
            if id == "G001" {
                return Ok("El texto no cumple con el formato establecido".to_owned());
            }
        }
        Ok(id.to_owned())
    }

    /// Consulta si la referencia dispone de loader para el tipo.
    pub fn has_metadata_for(&self, model_type: &str) -> Result<bool, ValidatorError> {
        Ok(self.metadata_for(model_type)?.has_metadata)
    }

    /// Devuelve la metadata efectiva; los tipos sin loader tienen metadata vacía.
    pub fn metadata_for(&self, model_type: &str) -> Result<Metadata, ValidatorError> {
        let mut metadata = metadata::metadata(self.profile(), model_type)?;
        if metadata.has_metadata {
            if let Some(extension) = &self.extension {
                extension.extend_metadata(model_type, &mut metadata);
            }
        }
        Ok(metadata)
    }

    /// Como [`Self::metadata_for`] durante la validación: sin extensión que
    /// modifique las reglas se presta la tabla, sin copiarla en cada nodo.
    /// `None` si el tipo no tiene loader.
    fn effective_metadata(
        &self,
        model_type: &str,
    ) -> Result<Option<Cow<'static, Metadata>>, ValidatorError> {
        let Some(metadata) = metadata::metadata_ref(self.profile(), model_type)?
            .filter(|metadata| metadata.has_metadata)
        else {
            return Ok(None);
        };
        Ok(Some(match &self.extension {
            Some(extension) => {
                let mut metadata = metadata.clone();
                extension.extend_metadata(model_type, &mut metadata);
                Cow::Owned(metadata)
            }
            None => Cow::Borrowed(metadata),
        }))
    }

    /// Valida un documento de `core` por su tipo Rust.
    pub fn validate_document<T: Document + Serialize>(
        &self,
        document: &T,
    ) -> Result<Vec<Violation>, ValidatorError> {
        self.validate_model(document)
    }

    /// Valida individualmente cualquier modelo de `core` que implementa `Serialize`.
    pub fn validate_model<T: Serialize>(
        &self,
        model: &T,
    ) -> Result<Vec<Violation>, ValidatorError> {
        let name = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or_default();
        self.validate_named(
            if name == "RelatedDocument" {
                "Document"
            } else {
                name
            },
            model,
        )
    }

    /// Valida con nombre explícito, útil para `PaymentTerms` y sus subtipos.
    pub fn validate_named<T: Serialize>(
        &self,
        model_type: &str,
        model: &T,
    ) -> Result<Vec<Violation>, ValidatorError> {
        let value = serde_json::to_value(model)
            .map_err(|error| ValidatorError::Model(error.to_string()))?;
        inspect_input(&value)?;
        let mut state = State {
            nodes: 0,
            violations: Vec::new(),
        };
        self.validate_value(model_type, &value, "", 0, true, &mut state)?;
        Ok(state.violations)
    }

    /// Valida un mapa de propiedades sin pasar por los tipos de `core`.
    /// Útil para caracterizar `Type` con entradas que un setter rechaza.
    pub fn validate_raw(
        &self,
        model_type: &str,
        fields: &Value,
    ) -> Result<Vec<Violation>, ValidatorError> {
        inspect_input(fields)?;
        let mut state = State {
            nodes: 0,
            violations: Vec::new(),
        };
        self.validate_value(model_type, fields, "", 0, false, &mut state)?;
        Ok(state.violations)
    }

    fn profile(&self) -> &str {
        if self.version.as_deref() == Some("2.1") {
            "2.1"
        } else {
            "default"
        }
    }

    fn validate_value(
        &self,
        model_type: &str,
        value: &Value,
        prefix: &str,
        depth: usize,
        typed: bool,
        state: &mut State,
    ) -> Result<(), ValidatorError> {
        if depth > MAX_DEPTH {
            return Err(ValidatorError::Limit("profundidad"));
        }
        state.nodes += 1;
        if state.nodes > MAX_NODES {
            return Err(ValidatorError::Limit("nodos"));
        }
        let Some(metadata) = self
            .effective_metadata(model_type)?
            .filter(|metadata| metadata.has_metadata)
        else {
            return Ok(());
        };
        if let Some(extension) = &self.extension {
            for mut violation in extension.validate(model_type, value) {
                violation.path = join_path(prefix, &violation.path);
                state.push(violation)?;
            }
        }
        for field in &metadata.fields {
            let field_value = value.get(&field.field).unwrap_or(&Value::Null);
            // La ruta solo se construye si hace falta: para una violación o
            // un modelo anidado.
            let path = || join_path(prefix, &field.field);
            for rule in &field.rules {
                if rule.name == "Valid" {
                    if !field_value.is_null() {
                        let child_type = nested::nested_type(model_type, &field.field, field_value);
                        if let Some(child_type) = child_type {
                            let path = path();
                            if let Some(items) = field_value.as_array() {
                                for (index, item) in items.iter().enumerate() {
                                    self.validate_value(
                                        child_type,
                                        item,
                                        &format!("{path}[{index}]"),
                                        depth + 1,
                                        typed,
                                        state,
                                    )?;
                                }
                            } else if field_value.is_object() {
                                self.validate_value(
                                    child_type,
                                    field_value,
                                    &path,
                                    depth + 1,
                                    typed,
                                    state,
                                )?;
                            }
                        }
                    }
                } else if let Some(violation) = self.check_rule(rule, field_value, &path, typed)? {
                    state.push(violation)?;
                }
            }
        }
        Ok(())
    }

    fn check_rule(
        &self,
        rule: &Rule,
        value: &Value,
        path: &dyn Fn() -> String,
        typed: bool,
    ) -> Result<Option<Violation>, ValidatorError> {
        let option = |key: &str| rule.options.get(key);
        let mut parameters = BTreeMap::new();
        let (template, code) = match rule.name.as_str() {
            "NotBlank" => {
                if option("allowNull").and_then(Value::as_bool) == Some(true) && value.is_null()
                    || !is_blank(value)
                {
                    return Ok(None);
                }
                parameters.insert("{{ value }}".to_owned(), format_value(value));
                (
                    option_text(rule, "message", "This value should not be blank."),
                    CODE_NOT_BLANK,
                )
            }
            "NotNull" => {
                if !value.is_null() {
                    return Ok(None);
                }
                parameters.insert("{{ value }}".to_owned(), "null".to_owned());
                (
                    option_text(rule, "message", "This value should not be null."),
                    CODE_NOT_NULL,
                )
            }
            "Length" => {
                if value.is_null() {
                    return Ok(None);
                }
                let Some(content) = scalar_string(value) else {
                    return Err(ValidatorError::Model(format!(
                        "Length requiere string en {}",
                        path()
                    )));
                };
                let count = content.chars().count();
                let min = option("min").and_then(Value::as_u64).map(|n| n as usize);
                let max = option("max").and_then(Value::as_u64).map(|n| n as usize);
                let (template, code, limit) =
                    if let Some(limit) = max.filter(|limit| count > *limit) {
                        if min == Some(limit) {
                            (
                                option_text(rule, "exactMessage", ""),
                                CODE_EXACT_LENGTH,
                                limit,
                            )
                        } else {
                            (option_text(rule, "maxMessage", ""), CODE_TOO_LONG, limit)
                        }
                    } else if let Some(limit) = min.filter(|limit| count < *limit) {
                        if max == Some(limit) {
                            (
                                option_text(rule, "exactMessage", ""),
                                CODE_EXACT_LENGTH,
                                limit,
                            )
                        } else {
                            (option_text(rule, "minMessage", ""), CODE_TOO_SHORT, limit)
                        }
                    } else {
                        return Ok(None);
                    };
                parameters.insert(
                    "{{ value }}".to_owned(),
                    format_value(&Value::String(content.into_owned())),
                );
                parameters.insert("{{ limit }}".to_owned(), limit.to_string());
                parameters.insert("{{ value_length }}".to_owned(), count.to_string());
                (template, code)
            }
            "Regex" => {
                if value.is_null() || value.as_str() == Some("") {
                    return Ok(None);
                }
                let Some(content) = scalar_string(value) else {
                    return Err(ValidatorError::Model(format!(
                        "Regex requiere string en {}",
                        path()
                    )));
                };
                let pattern = option_text(rule, "pattern", "");
                let valid = match pattern {
                    "/^[0-9]{11}$/" => {
                        content.len() == 11 && content.bytes().all(|byte| byte.is_ascii_digit())
                    }
                    "/^[0-9]{1,8}$/" => {
                        (1..=8).contains(&content.len())
                            && content.bytes().all(|byte| byte.is_ascii_digit())
                    }
                    _ => return Err(ValidatorError::UnsupportedRule(format!("Regex {pattern}"))),
                };
                if valid == option("match").and_then(Value::as_bool).unwrap_or(true) {
                    return Ok(None);
                }
                parameters.insert("{{ value }}".to_owned(), format_value(value));
                parameters.insert("{{ pattern }}".to_owned(), pattern.to_owned());
                (
                    option_text(rule, "message", "This value is not valid."),
                    CODE_REGEX,
                )
            }
            "Choice" => {
                if value.is_null() {
                    return Ok(None);
                }
                let choices = option("choices").and_then(Value::as_array).ok_or_else(|| {
                    ValidatorError::UnsupportedRule("Choice sin choices".to_owned())
                })?;
                if choices.contains(value) {
                    return Ok(None);
                }
                parameters.insert("{{ value }}".to_owned(), format_value(value));
                parameters.insert(
                    "{{ choices }}".to_owned(),
                    choices
                        .iter()
                        .map(format_value)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                (
                    option_text(
                        rule,
                        "message",
                        "The value you selected is not a valid choice.",
                    ),
                    CODE_CHOICE,
                )
            }
            "Type" => {
                if value.is_null() {
                    return Ok(None);
                }
                let kind = option_text(rule, "type", "");
                let valid = match kind {
                    "bool" => value.is_boolean(),
                    "float" => value.is_f64() || typed && value.as_str().is_some_and(is_numeric),
                    "numeric" => value.is_number() || value.as_str().is_some_and(is_numeric),
                    _ => return Err(ValidatorError::UnsupportedRule(format!("Type {kind}"))),
                };
                if valid {
                    return Ok(None);
                }
                parameters.insert("{{ value }}".to_owned(), format_value(value));
                parameters.insert("{{ type }}".to_owned(), kind.to_owned());
                (
                    option_text(rule, "message", "This value should be of type {{ type }}."),
                    CODE_TYPE,
                )
            }
            "Required" => return Ok(None), // Symfony 6.4: Existence sin constraints no añade violación en propiedad.
            unknown => return Err(ValidatorError::UnsupportedRule(unknown.to_owned())),
        };
        let message = if rule.name == "Length" {
            render_plural(template, &parameters)
        } else if rule.name == "Type" {
            template.replace("{{ type }}", option_text(rule, "type", ""))
        } else {
            self.translate(template, &parameters, Some("validators"), None)?
        };
        Ok(Some(Violation {
            path: path(),
            message,
            template: template.to_owned(),
            code: Some(code.to_owned()),
            parameters,
            invalid: value.clone(),
        }))
    }
}

fn inspect_input(value: &Value) -> Result<(), ValidatorError> {
    fn walk(
        value: &Value,
        depth: usize,
        nodes: &mut usize,
        bytes: &mut usize,
    ) -> Result<(), ValidatorError> {
        if depth > MAX_DEPTH {
            return Err(ValidatorError::Limit("profundidad"));
        }
        *nodes = nodes.saturating_add(1);
        if *nodes > MAX_NODES {
            return Err(ValidatorError::Limit("nodos"));
        }
        match value {
            Value::Null => *bytes = bytes.saturating_add(4),
            Value::Bool(_) => *bytes = bytes.saturating_add(5),
            Value::Number(number) => *bytes = bytes.saturating_add(number.to_string().len()),
            Value::String(content) => *bytes = bytes.saturating_add(content.len()),
            Value::Array(items) => {
                for item in items {
                    walk(item, depth + 1, nodes, bytes)?;
                }
            }
            Value::Object(fields) => {
                for (key, field) in fields {
                    *bytes = bytes.saturating_add(key.len());
                    walk(field, depth + 1, nodes, bytes)?;
                }
            }
        }
        if *bytes > MAX_INPUT_BYTES {
            return Err(ValidatorError::Limit("tamaño de entrada"));
        }
        Ok(())
    }
    walk(value, 0, &mut 0, &mut 0)
}

struct State {
    nodes: usize,
    violations: Vec<Violation>,
}
impl State {
    fn push(&mut self, violation: Violation) -> Result<(), ValidatorError> {
        if self.violations.len() >= MAX_VIOLATIONS {
            return Err(ValidatorError::Limit("violaciones"));
        }
        self.violations.push(violation);
        Ok(())
    }
}

fn join_path(prefix: &str, property: &str) -> String {
    if prefix.is_empty() {
        property.to_owned()
    } else {
        format!("{prefix}.{property}")
    }
}

fn option_text<'a>(rule: &'a Rule, key: &str, fallback: &'a str) -> &'a str {
    rule.options
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
}

fn is_blank(value: &Value) -> bool {
    value.is_null()
        || value == &Value::Bool(false)
        || value.as_str() == Some("")
        || value.as_array().is_some_and(Vec::is_empty)
}

/// Texto escalar como el cast `(string)` de la referencia; las cadenas se
/// prestan.
fn scalar_string(value: &Value) -> Option<Cow<'_, str>> {
    match value {
        Value::String(text) => Some(Cow::Borrowed(text)),
        Value::Number(number) => Some(Cow::Owned(number.to_string())),
        Value::Bool(true) => Some(Cow::Borrowed("1")),
        Value::Bool(false) => Some(Cow::Borrowed("")),
        _ => None,
    }
}

/// `ConstraintValidator::formatValue` de Symfony: las cadenas van entre
/// comillas sin escapar nada (`'"'.$value.'"'`) y los números como el cast
/// `(string)` de la referencia.
fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => match value.as_f64().filter(|_| value.is_f64()) {
            Some(float) => float_string(float),
            None => value.to_string(),
        },
        Value::String(value) => format!("\"{value}\""),
        Value::Array(_) => "array".to_owned(),
        Value::Object(_) => "object".to_owned(),
    }
}

/// `(string) $float` de la referencia con `precision=14` (`zend_gcvt`): 14 cifras
/// significativas correctamente redondeadas, sin ceros finales, y notación
/// `1.0E+15` si el punto decimal queda a más de 14 posiciones o antes de
/// `0.0001`.
fn float_string(value: f64) -> String {
    if value.is_nan() {
        return "NAN".to_owned();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-INF" } else { "INF" }.to_owned();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if value == 0.0 {
        return format!("{sign}0");
    }
    // `{:.13e}`: 14 cifras exactas redondeadas, p. ej. «1.0000000000000e15».
    let scientific = format!("{:.13e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let mut digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    let decpt = exponent.parse::<i32>().unwrap_or(0) + 1;
    let (first, rest) = digits.split_at(1);
    if !(-3..=14).contains(&decpt) {
        let exp = decpt - 1;
        let rest = if rest.is_empty() { "0" } else { rest };
        let exp_sign = if exp < 0 { '-' } else { '+' };
        format!("{sign}{first}.{rest}E{exp_sign}{}", exp.unsigned_abs())
    } else if decpt <= 0 {
        let zeros = "0".repeat(decpt.unsigned_abs() as usize);
        format!("{sign}0.{zeros}{digits}")
    } else {
        let point = decpt as usize;
        if digits.len() <= point {
            format!("{sign}{digits}{}", "0".repeat(point - digits.len()))
        } else {
            let (int, frac) = digits.split_at(point);
            format!("{sign}{int}.{frac}")
        }
    }
}

fn is_numeric(value: &str) -> bool {
    let bytes = value.trim().as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut index = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        index += 1;
    }
    let mut digits = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
        digits += 1;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return false;
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    index == bytes.len()
}

fn render_plural(template: &str, parameters: &BTreeMap<String, String>) -> String {
    let limit = parameters
        .get("{{ limit }}")
        .map(String::as_str)
        .unwrap_or("");
    let variant = if limit == "1" {
        template.split('|').next().unwrap_or(template)
    } else {
        template.split('|').nth(1).unwrap_or(template)
    };
    variant.replace("{{ limit }}", limit)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn float_strings() {
        // Valores obtenidos de la conversión `(string) $f` de la referencia.
        for (value, expected) in [
            (1e15, "1.0E+15"),
            (1e14, "1.0E+14"),
            (123_456_789_012_345.0, "1.2345678901234E+14"),
            (3.5, "3.5"),
            (3.0, "3"),
            (0.1 + 0.2, "0.3"),
            (0.0001, "0.0001"),
            (0.00001, "1.0E-5"),
            (-1.5e-7, "-1.5E-7"),
            (-0.0, "-0"),
            (99_999_999_999_999.0, "99999999999999"),
        ] {
            assert_eq!(super::float_string(value), expected, "{value}");
        }
    }

    #[test]
    fn strings_are_quoted_without_escaping() {
        assert_eq!(super::format_value(&json!("a\t\"b\"\\")), "\"a\t\"b\"\\\"");
        assert_eq!(super::format_value(&json!(1e15)), "1.0E+15");
        assert_eq!(super::format_value(&json!(7)), "7");
    }
}
