//! Filtros, funciones y tests de plantilla.
//!
//! - Del comprobante: filtros `catalog`, `image_b64`, `n_format`; funciones
//!   `legend`, `qrCode`, `qrCodeDespatch`, `qrUrl`.
//! - Conversión e impresión de valores ([`formatter`]) y versiones propias de
//!   `raw`, `upper`, `lower`, `length`, `default`, `escape`/`e` y `date` con
//!   esas reglas de conversión.
//! - Tests `twig_true`, `twig_in`, `twig_eq` y filtro `twig_string` para la
//!   verdad, la pertenencia, la igualdad laxa y la conversión a texto (ver
//!   el README del crate; los nombres forman parte del contrato de las
//!   plantillas).
//!
//! Los filtros del comprobante validan los tipos de sus argumentos: uno de
//! tipo incorrecto produce [`RenderCause::TypeError`].
//!
//! # Reglas de conversión
//!
//! - Texto ([`to_text`]): nulo → `""`, `true` → `"1"`, `false` → `""`,
//!   enteros en decimal, `f64` con 14 cifras ([`format::float_to_string`]),
//!   listas y mapas → `"Array"`; modelos y fechas no tienen texto.
//! - Verdad ([`is_truthy`]): nulo, `false`, `0`, `0.0`, `""`, `"0"` y las
//!   colecciones vacías son falsos.
//! - Igualdad laxa ([`loose_eq`]): dos cadenas numéricas, o un número y una
//!   cadena numérica, se comparan como números; un booleano compara verdades;
//!   nulo es igual a `""`, `0`, `false` y colecciones vacías.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use minijinja::value::{Enumerator, Kwargs, Object, ObjectRepr, Rest, ValueKind};
use minijinja::{
    AutoEscape, Environment, Error, ErrorKind, Output, State, UndefinedBehavior, Value,
};

use crate::catalog;
use crate::context::{DateObject, ModelObject};
use crate::error::{RenderCause, engine_error};
use crate::format::{self, Number};
use crate::qr::{self, SalePayloadParts};

/// Reloj e límites que necesitan las funciones durante un render.
#[derive(Debug, Clone)]
pub(crate) struct Runtime {
    /// Instante que usa `date` con una fecha nula.
    pub(crate) now: jiff::Timestamp,
}

/// Escapa `&`, `<`, `>`, `"` y `'` (como `&#039;`) y escribe en `out` por
/// tramos, sin copia intermedia.
pub(crate) fn write_escaped_html<W: fmt::Write + ?Sized>(out: &mut W, value: &str) -> fmt::Result {
    let mut start = 0;
    for (index, byte) in value.bytes().enumerate() {
        let replacement = match byte {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#039;",
            _ => continue,
        };
        out.write_str(&value[start..index])?;
        out.write_str(replacement)?;
        start = index + 1;
    }
    out.write_str(&value[start..])
}

/// [`write_escaped_html`] al final de una cadena.
pub(crate) fn escape_html_into(out: &mut String, value: &str) {
    // Escribir en un `String` no falla.
    let _ = write_escaped_html(out, value);
}

/// Resultado de un getter calculado que no se puede obtener (`getName()` sin
/// emisor). Falla al imprimirse o al pasarse a un filtro o función.
#[derive(Debug)]
pub(crate) struct GetterFailure(pub(crate) RenderCause);

impl Object for GetterFailure {}

/// Error de un [`GetterFailure`].
pub(crate) fn check(value: &Value) -> Result<(), Error> {
    match value.downcast_object_ref::<GetterFailure>() {
        Some(failure) => Err(engine_error(failure.0.clone())),
        None => Ok(()),
    }
}

fn is_strict(state: &State<'_, '_>) -> bool {
    state.undefined_behavior() == UndefinedBehavior::Strict
}

/// Con `strict_variables`, un valor indefinido que llega a un filtro o función
/// es un error.
fn defined(state: &State<'_, '_>, value: &Value) -> Result<(), Error> {
    if value.is_undefined() && is_strict(state) {
        return Err(Error::from(ErrorKind::UndefinedError));
    }
    check(value)
}

fn type_error(function: &'static str) -> Error {
    engine_error(RenderCause::TypeError { function })
}

/// Entero o `f64` del valor, si es un número.
fn number(value: &Value) -> Option<Number> {
    if value.kind() != ValueKind::Number {
        return None;
    }
    if value.is_integer() {
        if let Some(i) = value.as_i64() {
            return Some(Number::Int(i));
        }
    }
    f64::try_from(value.clone()).ok().map(Number::Float)
}

/// Texto de un valor según las reglas de conversión del módulo. Las cadenas
/// se prestan sin copiarse.
///
/// # Errors
///
/// [`RenderCause::NotPrintable`] para modelos y fechas.
pub(crate) fn to_text(value: &Value) -> Result<Cow<'_, str>, Error> {
    check(value)?;
    Ok(match value.kind() {
        ValueKind::String => Cow::Borrowed(value.as_str().unwrap_or_default()),
        ValueKind::Undefined | ValueKind::None => Cow::Borrowed(""),
        ValueKind::Bool => Cow::Borrowed(if value.is_true() { "1" } else { "" }),
        ValueKind::Number => Cow::Owned(match number(value) {
            Some(Number::Int(i)) => i.to_string(),
            Some(Number::Float(f)) => format::float_to_string(f),
            None => value.to_string(),
        }),
        ValueKind::Bytes => String::from_utf8_lossy(value.as_bytes().unwrap_or_default()),
        // Una colección impresa produce el texto fijo `Array`.
        ValueKind::Seq | ValueKind::Map | ValueKind::Iterable
            if value.downcast_object_ref::<ModelObject>().is_none()
                && value.downcast_object_ref::<DateObject>().is_none() =>
        {
            Cow::Borrowed("Array")
        }
        _ => return Err(engine_error(RenderCause::NotPrintable)),
    })
}

/// [`to_text`] como cadena propia.
pub(crate) fn to_text_owned(value: &Value) -> Result<String, Error> {
    to_text(value).map(Cow::into_owned)
}

/// Verdad de un valor: nulo, `false`, `0`, `0.0`, `""`, `"0"` y las
/// colecciones vacías son falsos.
pub(crate) fn is_truthy(value: &Value) -> bool {
    match value.kind() {
        ValueKind::String => !matches!(value.as_str(), Some("" | "0")),
        ValueKind::Bytes => !matches!(value.as_bytes(), Some(b"" | b"0")),
        _ => value.is_true(),
    }
}

/// Formateador del motor: imprime [`to_text`] y lo escapa salvo que el valor
/// sea seguro o el escape esté desactivado.
pub(crate) fn formatter(
    out: &mut Output<'_>,
    state: &mut State<'_, '_>,
    value: &Value,
) -> Result<(), Error> {
    if value.is_undefined() && is_strict(state) {
        return Err(Error::from(ErrorKind::UndefinedError));
    }
    let text = to_text(value)?;
    if value.is_safe() || matches!(state.auto_escape(), AutoEscape::None) {
        out.write_str(&text).map_err(Error::from)
    } else {
        write_escaped_html(out, &text).map_err(Error::from)
    }
}

/// Número de una cadena numérica: admite espacios iniciales y finales,
/// signo, decimales y exponente; sin parte decimal ni exponente es entero.
fn numeric_string(text: &str) -> Option<Number> {
    let trimmed = text
        .trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c'])
        .trim_end_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let int_start = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let int_digits = i - int_start;
    let mut is_float = false;
    let mut frac_digits = 0;
    if bytes.get(i) == Some(&b'.') {
        is_float = true;
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
            frac_digits += 1;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return None;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(bytes.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp_start = j;
        while bytes.get(j).is_some_and(u8::is_ascii_digit) {
            j += 1;
        }
        if j > exp_start {
            is_float = true;
            i = j;
        }
    }
    if i != bytes.len() {
        return None;
    }
    if !is_float {
        if let Ok(n) = trimmed.parse::<i64>() {
            return Some(Number::Int(n));
        }
    }
    trimmed.parse::<f64>().ok().map(Number::Float)
}

#[allow(
    clippy::float_arithmetic,
    clippy::cast_precision_loss,
    reason = "comparación numérica entre entero y f64"
)]
fn cmp_numbers(a: Number, b: Number) -> Option<Ordering> {
    match (a, b) {
        (Number::Int(x), Number::Int(y)) => Some(x.cmp(&y)),
        (Number::Int(x), Number::Float(y)) => (x as f64).partial_cmp(&y),
        (Number::Float(x), Number::Int(y)) => x.partial_cmp(&(y as f64)),
        (Number::Float(x), Number::Float(y)) => x.partial_cmp(&y),
    }
}

/// Igualdad laxa entre escalares (ver las reglas del módulo). Una cadena no
/// numérica frente a un número se compara con el texto del número; los
/// demás tipos, por igualdad estricta.
pub(crate) fn loose_eq(a: &Value, b: &Value) -> bool {
    let null = |v: &Value| v.is_none() || v.is_undefined();
    let text = |v: &Value| -> Option<String> {
        match v.kind() {
            ValueKind::String | ValueKind::Bytes => to_text(v).ok().map(Cow::into_owned),
            _ => None,
        }
    };
    if null(a) && null(b) {
        return true;
    }
    if a.kind() == ValueKind::Bool || b.kind() == ValueKind::Bool {
        return is_truthy(a) == is_truthy(b);
    }
    if null(a) || null(b) {
        let other = if null(a) { b } else { a };
        return match other.kind() {
            ValueKind::String | ValueKind::Bytes => text(other).is_some_and(|s| s.is_empty()),
            ValueKind::Number => !is_truthy(other),
            ValueKind::Seq | ValueKind::Map => other.len() == Some(0),
            _ => false,
        };
    }
    match (number(a), number(b), text(a), text(b)) {
        (Some(x), Some(y), _, _) => cmp_numbers(x, y) == Some(Ordering::Equal),
        (Some(x), None, _, Some(s)) | (None, Some(x), Some(s), _) => match numeric_string(&s) {
            Some(y) => cmp_numbers(x, y) == Some(Ordering::Equal),
            None => {
                let shown = match x {
                    Number::Int(i) => i.to_string(),
                    Number::Float(f) => format::float_to_string(f),
                };
                shown == s
            }
        },
        (None, None, Some(s), Some(t)) => match (numeric_string(&s), numeric_string(&t)) {
            (Some(x), Some(y)) => cmp_numbers(x, y) == Some(Ordering::Equal),
            _ => s == t,
        },
        _ => a == b,
    }
}

/// Pertenencia laxa: subcadena si `compare` es texto; si es una colección,
/// algún elemento (o valor de un mapa) laxamente igual a `value`. Los
/// objetos solo coinciden consigo mismos.
fn loose_contains(value: &Value, compare: &Value) -> Result<bool, Error> {
    check(compare)?;
    check(value)?;
    let (value, compare) = (value.clone(), compare.clone());
    if compare.kind() == ValueKind::String {
        return Ok(match value.kind() {
            ValueKind::String | ValueKind::Number => {
                let needle = to_text_owned(&value)?;
                needle.is_empty() || compare.as_str().unwrap_or_default().contains(&needle)
            }
            _ => false,
        });
    }
    if !matches!(
        compare.kind(),
        ValueKind::Seq | ValueKind::Map | ValueKind::Iterable
    ) {
        return Ok(false);
    }
    let Ok(items) = compare.try_iter() else {
        return Ok(false);
    };
    let is_map = compare.kind() == ValueKind::Map;
    for item in items {
        let item = if is_map {
            compare.get_item(&item).unwrap_or_default()
        } else {
            item
        };
        if value.as_object().is_some() || item.as_object().is_some() {
            if value.as_object().is_some() && item.as_object().is_some() && value == item {
                return Ok(true);
            }
            continue;
        }
        if loose_eq(&value, &item) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Mapa con orden de inserción; las claves se comparan como texto.
#[derive(Debug)]
pub(crate) struct OrderedMap(pub(crate) Vec<(String, Value)>);

impl Object for OrderedMap {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Map
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let key = to_text(key).ok()?;
        self.0
            .iter()
            .find(|(k, _)| *k == *key)
            .map(|(_, v)| v.clone())
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Values(
            self.0
                .iter()
                .map(|(k, _)| Value::from(k.as_str()))
                .collect(),
        )
    }

    fn is_true(self: &Arc<Self>) -> bool {
        !self.0.is_empty()
    }
}

/// `ParamValue` → valor de plantilla.
pub(crate) fn param_value(value: &crate::ParamValue) -> Value {
    use crate::ParamValue as P;
    match value {
        P::Null => Value::from(()),
        P::Bool(b) => Value::from(*b),
        P::Int(i) => Value::from(*i),
        P::Decimal(d) => Value::from(format::decimal_to_f64(*d)),
        P::Text(s) => Value::from(s.as_str()),
        P::Bytes(b) => Value::from_bytes(b.clone()),
        P::List(items) => Value::from(items.iter().map(param_value).collect::<Vec<_>>()),
        P::Map(pairs) => Value::from_object(OrderedMap(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), param_value(v)))
                .collect(),
        )),
    }
}

/// Clave de búsqueda del filtro `catalog`: nulo → `""`, booleano → `"1"`/`"0"`,
/// `f64` truncado a entero, cadenas tal cual; otro tipo es un error de tipo.
fn array_key(value: &Value) -> Result<String, Error> {
    Ok(match value.kind() {
        ValueKind::Undefined | ValueKind::None => String::new(),
        ValueKind::Bool => if value.is_true() { "1" } else { "0" }.to_owned(),
        ValueKind::Number => match number(value) {
            Some(Number::Int(i)) => i.to_string(),
            #[allow(
                clippy::cast_possible_truncation,
                reason = "una clave f64 se trunca a entero"
            )]
            Some(Number::Float(f)) if f.is_finite() => (f.trunc() as i64).to_string(),
            _ => "0".to_owned(),
        },
        ValueKind::String | ValueKind::Bytes => to_text_owned(value)?,
        _ => return Err(type_error("DocumentFilter::getValueCatalog")),
    })
}

fn catalog(state: &State<'_, '_>, value: Value, code: Option<Value>) -> Result<Value, Error> {
    defined(state, &value)?;
    let code = code.unwrap_or_default();
    defined(state, &code)?;
    let (value, code) = (array_key(&value)?, array_key(&code)?);
    Ok(Value::from(catalog::lookup(&value, &code)))
}

/// Subtipo MIME de una imagen: `png` por firma, `gif` por prefijo y `jpeg`
/// en cualquier otro caso.
#[must_use]
pub fn mime_type(raw: &[u8]) -> &'static str {
    if raw.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if raw.starts_with(b"GIF") {
        "gif"
    } else {
        "jpeg"
    }
}

/// URI `data:image/<mime>;base64,<datos>`. Sin `mime` (o con `""` o `"0"`) se
/// detecta con [`mime_type`].
#[must_use]
pub fn image_data_uri(image: &[u8], mime: Option<&str>) -> String {
    let mime = mime
        .filter(|m| !m.is_empty() && *m != "0")
        .unwrap_or_else(|| mime_type(image));
    let prefix_len = "data:image/;base64,".len() + mime.len();
    let encoded_len = base64::encoded_len(image.len(), true).unwrap_or(0);
    let mut out = String::with_capacity(prefix_len.saturating_add(encoded_len));
    out.push_str("data:image/");
    out.push_str(mime);
    out.push_str(";base64,");
    STANDARD.encode_string(image, &mut out);
    out
}

/// Bytes de una cadena o cadena binaria, prestados.
fn bytes_of(value: &Value) -> Option<&[u8]> {
    match value.kind() {
        ValueKind::String | ValueKind::Bytes => value.as_bytes(),
        _ => None,
    }
}

fn image_b64(state: &State<'_, '_>, image: Value, mime: Option<Value>) -> Result<String, Error> {
    defined(state, &image)?;
    let mime = mime.unwrap_or_else(|| Value::from(""));
    defined(state, &mime)?;
    // Un MIME falso ('', '0', nulo, false, 0, 0.0, []) usa la autodetección.
    let explicit = if is_truthy(&mime) {
        Some(to_text_owned(&mime)?)
    } else {
        None
    };
    let function = if explicit.is_some() {
        "base64_encode"
    } else {
        "substr"
    };
    let bytes = bytes_of(&image).ok_or_else(|| type_error(function))?;
    Ok(image_data_uri(bytes, explicit.as_deref()))
}

fn n_format(state: &State<'_, '_>, value: Value, decimals: Option<Value>) -> Result<String, Error> {
    defined(state, &value)?;
    let decimals = match decimals {
        None => 2,
        Some(d) => {
            defined(state, &d)?;
            match number(&d) {
                Some(Number::Int(i)) => i,
                _ => return Err(type_error("number_format")),
            }
        }
    };
    let number = number(&value).ok_or_else(|| type_error("number_format"))?;
    Ok(format::number_format(number, decimals))
}

fn raw(state: &State<'_, '_>, value: Value) -> Result<Value, Error> {
    defined(state, &value)?;
    Ok(Value::from_safe_string(to_text_owned(&value)?))
}

fn upper(state: &State<'_, '_>, value: Value) -> Result<String, Error> {
    defined(state, &value)?;
    Ok(to_text(&value)?.to_uppercase())
}

fn lower(state: &State<'_, '_>, value: Value) -> Result<String, Error> {
    defined(state, &value)?;
    Ok(to_text(&value)?.to_lowercase())
}

fn escape(state: &State<'_, '_>, value: Value, _rest: Rest<Value>) -> Result<Value, Error> {
    defined(state, &value)?;
    if value.is_safe() {
        return Ok(value);
    }
    let text = to_text(&value)?;
    let mut out = String::with_capacity(text.len());
    escape_html_into(&mut out, &text);
    Ok(Value::from_safe_string(out))
}

/// Longitud: 0 para nulo, elementos de una colección, 1 para un objeto y
/// caracteres del texto en otro caso.
fn length(state: &State<'_, '_>, value: Value) -> Result<usize, Error> {
    defined(state, &value)?;
    check(&value)?;
    Ok(match value.kind() {
        ValueKind::Undefined | ValueKind::None => 0,
        ValueKind::Seq | ValueKind::Map | ValueKind::Iterable
            if value.downcast_object_ref::<ModelObject>().is_none()
                && value.downcast_object_ref::<DateObject>().is_none() =>
        {
            value
                .len()
                .unwrap_or_else(|| value.try_iter().map_or(0, Iterator::count))
        }
        ValueKind::Plain => 1,
        _ => to_text(&value)?.chars().count(),
    })
}

/// Sustituye `""`, nulo, indefinido, `false` y las colecciones vacías por
/// `other` (o `""`).
fn default_filter(
    state: &State<'_, '_>,
    value: Value,
    other: Option<Value>,
) -> Result<Value, Error> {
    let _ = state;
    let empty = match value.kind() {
        ValueKind::Undefined | ValueKind::None => true,
        ValueKind::String => value.as_str() == Some(""),
        ValueKind::Bool => !value.is_true(),
        ValueKind::Seq | ValueKind::Map => value.len() == Some(0),
        _ => false,
    };
    Ok(if empty {
        other.unwrap_or_else(|| Value::from(""))
    } else {
        value
    })
}

fn text_filter(state: &State<'_, '_>, value: Value) -> Result<String, Error> {
    defined(state, &value)?;
    to_text_owned(&value)
}

fn date_filter(
    state: &State<'_, '_>,
    value: Value,
    format: Option<Value>,
    _kwargs: Kwargs,
) -> Result<String, Error> {
    defined(state, &value)?;
    let format = match format {
        Some(f) if !f.is_none() && !f.is_undefined() => to_text_owned(&f)?,
        // Formato por defecto.
        _ => "F j, Y H:i".to_owned(),
    };
    let lima = cpeforge_core::time::peru_time_zone()
        .map_err(|e| engine_error(RenderCause::Document(e)))?;
    let zoned = if let Some(date) = value.downcast_object_ref::<DateObject>() {
        date.0.with_time_zone(lima)
    } else if value.is_none() || value.is_undefined() {
        let now = state
            .lookup(RUNTIME_KEY)
            .and_then(|v| v.downcast_object_ref::<RuntimeValue>().map(|r| r.0.now))
            .unwrap_or_else(jiff::Timestamp::now);
        now.to_zoned(lima)
    } else if let Some(Number::Int(seconds)) = number(&value) {
        // Un entero son segundos desde la época Unix.
        jiff::Timestamp::from_second(seconds)
            .map_err(|_| type_error("date"))?
            .to_zoned(lima)
    } else {
        // No se interpretan cadenas de fecha.
        return Err(type_error("date"));
    };
    Ok(format::format_date(&zoned, &format))
}

/// Clave interna del contexto con el [`Runtime`].
pub(crate) const RUNTIME_KEY: &str = "__cpeforge_report_runtime";

/// [`Runtime`] dentro del contexto.
#[derive(Debug)]
pub(crate) struct RuntimeValue(pub(crate) Runtime);

impl Object for RuntimeValue {}

fn legend(state: &State<'_, '_>, legends: Value, code: Value) -> Result<Value, Error> {
    defined(state, &legends)?;
    defined(state, &code)?;
    // Leyendas que no son una colección: cadena vacía.
    if !matches!(
        legends.kind(),
        ValueKind::Seq | ValueKind::Map | ValueKind::Iterable
    ) || legends.downcast_object_ref::<ModelObject>().is_some()
    {
        return Ok(Value::from(""));
    }
    for item in legends.try_iter()? {
        let object = item.downcast_object_ref::<ModelObject>().ok_or_else(|| {
            engine_error(RenderCause::UndefinedMethod {
                class: item.kind().to_string(),
                method: "getCode",
            })
        })?;
        let undefined = |method| {
            engine_error(RenderCause::UndefinedMethod {
                class: object.class().to_owned(),
                method,
            })
        };
        let item_code = object.call("getCode").ok_or_else(|| undefined("getCode"))?;
        if loose_eq(&item_code, &code) {
            return object.call("getValue").ok_or_else(|| undefined("getValue"));
        }
    }
    Ok(Value::from(""))
}

/// Valor del getter `method`; falla si el modelo no lo tiene o si el getter
/// calculado no se puede obtener.
fn getter(object: &ModelObject, method: &'static str) -> Result<Value, Error> {
    let value = object.call(method).ok_or_else(|| {
        engine_error(RenderCause::UndefinedMethod {
            class: object.class().to_owned(),
            method,
        })
    })?;
    check(&value)?;
    Ok(value)
}

/// `$x->method()` sobre el resultado de otro getter: falla si es `null`.
fn member<'a>(
    value: &'a Value,
    model: &'static str,
    field: &'static str,
) -> Result<&'a ModelObject, Error> {
    value.downcast_object_ref::<ModelObject>().ok_or_else(|| {
        engine_error(RenderCause::Document(
            cpeforge_core::CoreError::MissingField { model, field },
        ))
    })
}

fn opt_str(value: &Value) -> Result<Option<String>, Error> {
    if value.is_none() || value.is_undefined() {
        Ok(None)
    } else {
        to_text_owned(value).map(Some)
    }
}

/// Importe con dos decimales para el contenido del QR; un valor nulo o no
/// numérico cuenta como 0.
fn amount2(value: &Value) -> String {
    match number(value) {
        Some(n) => format::number_format(n, 2),
        None => format::number_format(Number::Int(0), 2),
    }
}

fn ymd(value: &Value, model: &'static str, field: &'static str) -> Result<String, Error> {
    let date = value.downcast_object_ref::<DateObject>().ok_or_else(|| {
        engine_error(RenderCause::Document(
            cpeforge_core::CoreError::MissingField { model, field },
        ))
    })?;
    Ok(format::format_date(&date.0, "Y-m-d"))
}

fn root(value: &Value) -> Result<&ModelObject, Error> {
    value.downcast_object_ref::<ModelObject>().ok_or_else(|| {
        engine_error(RenderCause::UndefinedMethod {
            class: value.kind().to_string(),
            method: "getCompany",
        })
    })
}

fn qr_svg_value(content: &[u8]) -> Result<Value, Error> {
    qr::svg(content)
        .map(Value::from)
        .map_err(|e| engine_error(RenderCause::Qr(e)))
}

/// Contenido QR de una venta leído de un objeto de plantilla.
pub(crate) fn sale_payload_of(sale: &Value) -> Result<String, Error> {
    let sale = root(sale)?;
    let class = sale.class();
    let client = getter(sale, "getClient")?;
    let company = getter(sale, "getCompany")?;
    let ruc = opt_str(&getter(member(&company, class, "company")?, "getRuc")?)?;
    let tipo_doc = opt_str(&getter(sale, "getTipoDoc")?)?;
    let serie = opt_str(&getter(sale, "getSerie")?)?;
    let correlativo = opt_str(&getter(sale, "getCorrelativo")?)?;
    let igv = amount2(&getter(sale, "getMtoIGV")?);
    let total = amount2(&getter(sale, "getMtoImpVenta")?);
    let fecha = ymd(&getter(sale, "getFechaEmision")?, class, "fechaEmision")?;
    let client = member(&client, class, "client")?;
    let client_tipo = opt_str(&getter(client, "getTipoDoc")?)?;
    let client_num = opt_str(&getter(client, "getNumDoc")?)?;
    Ok(qr::sale_payload(&SalePayloadParts {
        ruc: ruc.as_deref(),
        tipo_doc: tipo_doc.as_deref(),
        serie: serie.as_deref(),
        correlativo: correlativo.as_deref(),
        igv,
        total,
        fecha,
        client_tipo_doc: client_tipo.as_deref(),
        client_num_doc: client_num.as_deref(),
    }))
}

/// Contenido QR de una guía leído de un objeto de plantilla.
pub(crate) fn despatch_payload_of(despatch: &Value) -> Result<String, Error> {
    let despatch = root(despatch)?;
    let class = despatch.class();
    let destinatario = getter(despatch, "getDestinatario")?;
    let company = getter(despatch, "getCompany")?;
    let mut parts = vec![
        opt_str(&getter(member(&company, class, "company")?, "getRuc")?)?,
        opt_str(&getter(despatch, "getTipoDoc")?)?,
        opt_str(&getter(despatch, "getSerie")?)?,
        opt_str(&getter(despatch, "getCorrelativo")?)?,
        Some(ymd(
            &getter(despatch, "getFechaEmision")?,
            class,
            "fechaEmision",
        )?),
    ];
    let destinatario = member(&destinatario, class, "destinatario")?;
    parts.push(opt_str(&getter(destinatario, "getTipoDoc")?)?);
    parts.push(opt_str(&getter(destinatario, "getNumDoc")?)?);
    let mut out = String::new();
    for part in parts {
        out.push_str(part.as_deref().unwrap_or_default());
        out.push('|');
    }
    Ok(out)
}

fn qr_code(state: &State<'_, '_>, sale: Value) -> Result<Value, Error> {
    defined(state, &sale)?;
    qr_svg_value(sale_payload_of(&sale)?.as_bytes())
}

fn qr_code_despatch(state: &State<'_, '_>, despatch: Value) -> Result<Value, Error> {
    defined(state, &despatch)?;
    qr_svg_value(despatch_payload_of(&despatch)?.as_bytes())
}

/// QR de una URL: acepta texto, números y booleanos (convertidos a texto);
/// nulo y otros tipos son un error de tipo.
fn qr_url(state: &State<'_, '_>, url: Value) -> Result<Value, Error> {
    defined(state, &url)?;
    let bytes: Cow<'_, [u8]> = match url.kind() {
        ValueKind::String | ValueKind::Bytes => Cow::Borrowed(bytes_of(&url).unwrap_or_default()),
        ValueKind::Number | ValueKind::Bool => Cow::Owned(to_text_owned(&url)?.into_bytes()),
        _ => return Err(type_error("CPEForge\\Report\\Render\\QrRender::getQrUrl")),
    };
    qr_svg_value(&bytes)
}

fn test_true(value: &Value) -> bool {
    is_truthy(value)
}

fn test_in(state: &State<'_, '_>, value: Value, compare: Value) -> Result<bool, Error> {
    defined(state, &value)?;
    defined(state, &compare)?;
    loose_contains(&value, &compare)
}

fn test_eq(state: &State<'_, '_>, value: Value, other: Value) -> Result<bool, Error> {
    defined(state, &value)?;
    defined(state, &other)?;
    check(&value)?;
    check(&other)?;
    Ok(loose_eq(&value, &other))
}

/// Registra filtros, tests, funciones y formateador.
pub(crate) fn register(env: &mut Environment<'_>) {
    env.set_formatter(formatter);
    env.add_filter("catalog", catalog);
    env.add_filter("image_b64", image_b64);
    env.add_filter("n_format", n_format);
    env.add_filter("raw", raw);
    env.add_filter("upper", upper);
    env.add_filter("lower", lower);
    env.add_filter("escape", escape);
    env.add_filter("e", escape);
    env.add_filter("length", length);
    env.add_filter("default", default_filter);
    env.add_filter("date", date_filter);
    env.add_filter("twig_string", text_filter);
    env.add_function("legend", legend);
    env.add_function("qrCode", qr_code);
    env.add_function("qrCodeDespatch", qr_code_despatch);
    env.add_function("qrUrl", qr_url);
    env.add_test("twig_true", test_true);
    env.add_test("twig_in", test_in);
    env.add_test("twig_eq", test_eq);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loose_equality_like_reference() {
        let s = |v: &str| Value::from(v);
        assert!(loose_eq(&s("1"), &s("01")));
        assert!(loose_eq(&s("1000.0"), &s("1000")));
        assert!(loose_eq(&s(" 1"), &s("1")));
        assert!(!loose_eq(&s("abc"), &s("ABC")));
        assert!(!loose_eq(&Value::from(()), &s("0")));
        assert!(loose_eq(&Value::from(()), &s("")));
        assert!(loose_eq(&Value::from(0), &Value::from(())));
        assert!(!loose_eq(&Value::from(0), &s("a")));
        assert!(loose_eq(&Value::from(18.0), &s("18")));
    }

    #[test]
    fn mime_detection() {
        assert_eq!(mime_type(b"\x89PNG\r\n\x1a\nrest"), "png");
        assert_eq!(mime_type(b"GIF89a"), "gif");
        assert_eq!(mime_type(b"\xff\xd8\xff"), "jpeg");
        assert_eq!(mime_type(b""), "jpeg");
        assert_eq!(
            image_data_uri(b"<svg/>", Some("svg+xml")),
            "data:image/svg+xml;base64,PHN2Zy8+"
        );
    }
}
