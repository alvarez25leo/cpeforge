//! Modelos de datos de la API GRE (esquemas del OpenAPI).
//!
//! Todos los campos son opcionales: ninguno es obligatorio al construir un
//! modelo y los ausentes quedan `None`. La lectura de respuestas es
//! tolerante con el tipo de cada valor:
//!
//! - un campo `null` o ausente queda `None`;
//! - los campos de texto aceptan cualquier valor salvo un objeto, que es un
//!   error (un número `0` es `"0"`, `true` es `"1"`, `false` es `""` y una
//!   lista es `"Array"`);
//! - `expires_in` toma el entero inicial de una cadena (`"3600"` y
//!   `"3600s"` son `3600`);
//! - un valor cadena en lugar de objeto se vuelve a decodificar como JSON;
//! - `fecRecepcion` vacía (`""`, `"0"`, `0`, `false`) produce el error
//!   `non-nullable fec_recepcion cannot be null`: el campo no admite un valor
//!   vacío.
//!
//! La serialización omite los campos `None`, escribe los demás en el orden
//! del esquema y escapa `/` y los caracteres no ASCII (véase
//! [`Model::to_json`]).

use std::fmt;

use serde_json::{Map, Value};

use crate::codec::{self, DecodeError, Json, present_field};
use crate::datetime::{DateTimeError, DateTimeValue};

const CPE_ERROR_ARRAY: &str = "\\CPEForge\\Sunat\\GRE\\Model\\CpeError[]";

mod sealed {
    /// Conversión interna a [`crate::codec::Json`]; impide implementar
    /// [`super::Model`] fuera del crate.
    #[allow(unreachable_pub, reason = "patrón de trait sellado")]
    pub trait ToJsonValue {
        fn to_json_value(&self) -> crate::codec::Json;
    }
}

use sealed::ToJsonValue;

/// Representación JSON de un modelo.
pub trait Model: ToJsonValue {
    /// Nombre del esquema en el OpenAPI.
    fn model_name(&self) -> &'static str;

    /// JSON compacto (`/` escapado, no ASCII como `\uXXXX`); es el cuerpo que
    /// se envía con `Content-Type` JSON.
    fn to_json(&self) -> String {
        codec::encode_compact_json(&self.to_json_value())
    }

    /// JSON con sangría de 4 espacios y el mismo escape; es el cuerpo que se
    /// envía con un `Content-Type` que no es JSON.
    fn to_json_pretty(&self) -> String {
        codec::encode_pretty_json(&self.to_json_value())
    }
}

fn push_str(fields: &mut Vec<(String, Json)>, name: &str, value: Option<&String>) {
    if let Some(v) = value {
        fields.push((name.to_owned(), Json::Str(v.clone())));
    }
}

fn string_field(
    src: Option<&Map<String, Value>>,
    name: &str,
) -> Result<Option<String>, DecodeError> {
    present_field(src, name)
        .map(codec::coerce_string)
        .transpose()
}

/// Token OAuth2 (`ApiToken`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ApiToken {
    /// `access_token`.
    pub access_token: Option<String>,
    /// `token_type`.
    pub token_type: Option<String>,
    /// `expires_in` en segundos, como entero de 64 bits aunque el OpenAPI
    /// declare `int32`.
    pub expires_in: Option<i64>,
}

impl fmt::Debug for ApiToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiToken")
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<oculto>"),
            )
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

impl ApiToken {
    pub(crate) fn from_source(src: Option<&Map<String, Value>>) -> Result<Self, DecodeError> {
        Ok(Self {
            access_token: string_field(src, "access_token")?,
            token_type: string_field(src, "token_type")?,
            expires_in: present_field(src, "expires_in").map(codec::coerce_int),
        })
    }
}

impl Model for ApiToken {
    fn model_name(&self) -> &'static str {
        "ApiToken"
    }
}

impl ToJsonValue for ApiToken {
    fn to_json_value(&self) -> Json {
        let mut f = Vec::new();
        push_str(&mut f, "access_token", self.access_token.as_ref());
        push_str(&mut f, "token_type", self.token_type.as_ref());
        if let Some(e) = self.expires_in {
            f.push(("expires_in".to_owned(), Json::Int(e)));
        }
        Json::Object(f)
    }
}

/// Documento enviado (`CpeDocument`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpeDocument {
    /// `archivo`.
    pub archivo: Option<CpeDocumentArchivo>,
}

impl Model for CpeDocument {
    fn model_name(&self) -> &'static str {
        "CpeDocument"
    }
}

impl ToJsonValue for CpeDocument {
    fn to_json_value(&self) -> Json {
        let mut f = Vec::new();
        if let Some(a) = &self.archivo {
            f.push(("archivo".to_owned(), a.to_json_value()));
        }
        Json::Object(f)
    }
}

/// Archivo ZIP en Base64 y su hash (`CpeDocumentArchivo`). El llamador
/// aporta los tres valores; este crate no comprime ni calcula el hash.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CpeDocumentArchivo {
    /// `nomArchivo`: nombre del ZIP (`RRRRRRRRRRR-TT-SSSS-NNNNNNNN.zip`).
    pub nom_archivo: Option<String>,
    /// `arcGreZip`: ZIP en Base64.
    pub arc_gre_zip: Option<String>,
    /// `hashZip`: SHA-256 del ZIP en hexadecimal.
    pub hash_zip: Option<String>,
}

impl fmt::Debug for CpeDocumentArchivo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CpeDocumentArchivo")
            .field(
                "nom_archivo",
                &self.nom_archivo.as_ref().map(|_| "<oculto>"),
            )
            .field(
                "arc_gre_zip",
                &self
                    .arc_gre_zip
                    .as_ref()
                    .map(|z| format!("<{} caracteres>", z.len())),
            )
            .field("hash_zip", &self.hash_zip.as_ref().map(|_| "<oculto>"))
            .finish()
    }
}

impl Model for CpeDocumentArchivo {
    fn model_name(&self) -> &'static str {
        "CpeDocument_archivo"
    }
}

impl ToJsonValue for CpeDocumentArchivo {
    fn to_json_value(&self) -> Json {
        let mut f = Vec::new();
        push_str(&mut f, "nomArchivo", self.nom_archivo.as_ref());
        push_str(&mut f, "arcGreZip", self.arc_gre_zip.as_ref());
        push_str(&mut f, "hashZip", self.hash_zip.as_ref());
        Json::Object(f)
    }
}

/// Error de servidor (`CpeError`, HTTP 500, y elementos de `errors`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CpeError {
    /// `cod`.
    pub cod: Option<String>,
    /// `msg`.
    pub msg: Option<String>,
}

impl fmt::Debug for CpeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CpeError(<contenido oculto>)")
    }
}

impl CpeError {
    pub(crate) fn from_source(src: Option<&Map<String, Value>>) -> Result<Self, DecodeError> {
        Ok(Self {
            cod: string_field(src, "cod")?,
            msg: string_field(src, "msg")?,
        })
    }
}

impl Model for CpeError {
    fn model_name(&self) -> &'static str {
        "CpeError"
    }
}

impl ToJsonValue for CpeError {
    fn to_json_value(&self) -> Json {
        let mut f = Vec::new();
        push_str(&mut f, "cod", self.cod.as_ref());
        push_str(&mut f, "msg", self.msg.as_ref());
        Json::Object(f)
    }
}

/// Error de validación (`CpeErrorValidation`, HTTP 422).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CpeErrorValidation {
    /// `cod`.
    pub cod: Option<String>,
    /// `msg`.
    pub msg: Option<String>,
    /// `errors`. Un elemento `null` del JSON se conserva como `None`, de modo
    /// que los índices coinciden con los del arreglo recibido.
    pub errors: Option<Vec<Option<CpeError>>>,
}

impl fmt::Debug for CpeErrorValidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CpeErrorValidation(<contenido oculto>)")
    }
}

impl CpeErrorValidation {
    pub(crate) fn from_source(src: Option<&Map<String, Value>>) -> Result<Self, DecodeError> {
        // Los campos se leen en el orden del esquema: el primero inválido
        // determina el error.
        let cod = string_field(src, "cod")?;
        let msg = string_field(src, "msg")?;
        let errors = match present_field(src, "errors") {
            None => None,
            Some(value) => {
                let decoded;
                let value = match value {
                    Value::String(s) => {
                        decoded = codec::decode_json_or_null(s.as_bytes());
                        &decoded
                    }
                    other => other,
                };
                let Value::Array(items) = value else {
                    return Err(DecodeError {
                        class: "InvalidArgumentException",
                        message: format!("Invalid array '{CPE_ERROR_ARRAY}'"),
                    });
                };
                let mut list = Vec::with_capacity(items.len());
                for item in items {
                    list.push(if item.is_null() {
                        None
                    } else {
                        Some(CpeError::from_source(codec::model_source(item).as_ref())?)
                    });
                }
                Some(list)
            }
        };
        Ok(Self { cod, msg, errors })
    }

    /// Errores presentes (sin los `null` del arreglo).
    pub fn errors_iter(&self) -> impl Iterator<Item = &CpeError> {
        self.errors.iter().flatten().flatten()
    }
}

impl Model for CpeErrorValidation {
    fn model_name(&self) -> &'static str {
        "CpeErrorValidation"
    }
}

impl ToJsonValue for CpeErrorValidation {
    fn to_json_value(&self) -> Json {
        let mut f = Vec::new();
        push_str(&mut f, "cod", self.cod.as_ref());
        push_str(&mut f, "msg", self.msg.as_ref());
        if let Some(errors) = &self.errors {
            f.push((
                "errors".to_owned(),
                Json::Array(
                    errors
                        .iter()
                        .map(|e| e.as_ref().map_or(Json::Null, ToJsonValue::to_json_value))
                        .collect(),
                ),
            ));
        }
        Json::Object(f)
    }
}

/// Respuesta del envío (`CpeResponse`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CpeResponse {
    /// `numTicket` (UUID).
    pub num_ticket: Option<String>,
    /// `fecRecepcion` (`date-time`), con el texto recibido.
    pub fec_recepcion: Option<DateTimeValue>,
}

impl fmt::Debug for CpeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CpeResponse(<contenido oculto>)")
    }
}

impl CpeResponse {
    pub(crate) fn from_source(src: Option<&Map<String, Value>>) -> Result<Self, DecodeError> {
        let num_ticket = string_field(src, "numTicket")?;
        let fec_recepcion = match present_field(src, "fecRecepcion") {
            None => None,
            Some(v) if codec::is_empty_value(v) => {
                return Err(DecodeError {
                    class: "InvalidArgumentException",
                    message: "non-nullable fec_recepcion cannot be null".to_owned(),
                });
            }
            Some(Value::Array(_)) => return Err(datetime_type_error("array")),
            Some(Value::Object(_)) => return Err(datetime_type_error("stdClass")),
            Some(v) => {
                let value = DateTimeValue::new(codec::coerce_string(v)?);
                // Un texto sin forma de fecha invalida la respuesta completa;
                // las fechas sin desplazamiento o fuera de rango se conservan
                // sin interpretar (véase `crate::datetime`).
                if value.parse() == Err(DateTimeError::Format) {
                    return Err(DecodeError {
                        class: "DateMalformedStringException",
                        message: format!("Failed to parse time string ({})", value.raw()),
                    });
                }
                Some(value)
            }
        };
        Ok(Self {
            num_ticket,
            fec_recepcion,
        })
    }
}

fn datetime_type_error(given: &str) -> DecodeError {
    DecodeError {
        class: "TypeError",
        message: format!(
            "DateTime::__construct(): Argument #1 ($datetime) must be of type string, {given} given"
        ),
    }
}

impl Model for CpeResponse {
    fn model_name(&self) -> &'static str {
        "CpeResponse"
    }
}

impl ToJsonValue for CpeResponse {
    fn to_json_value(&self) -> Json {
        let mut f = Vec::new();
        push_str(&mut f, "numTicket", self.num_ticket.as_ref());
        if let Some(d) = &self.fec_recepcion {
            let text = d
                .parse()
                .map_or_else(|_| d.raw().to_owned(), |p| p.format_atom());
            f.push(("fecRecepcion".to_owned(), Json::Str(text)));
        }
        Json::Object(f)
    }
}

/// Estado del envío (`StatusResponse`). `cod_respuesta` es una **cadena**:
/// `"98"` en proceso, `"99"` con error y `"0"` correcto.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct StatusResponse {
    /// `codRespuesta`.
    pub cod_respuesta: Option<String>,
    /// `error` (solo si `codRespuesta` es `"99"`).
    pub error: Option<StatusResponseError>,
    /// `arcCdr`: CDR en Base64.
    pub arc_cdr: Option<String>,
    /// `indCdrGenerado`: `"1"` si se generó CDR.
    pub ind_cdr_generado: Option<String>,
}

impl fmt::Debug for StatusResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StatusResponse(<contenido oculto>)")
    }
}

impl StatusResponse {
    pub(crate) fn from_source(src: Option<&Map<String, Value>>) -> Result<Self, DecodeError> {
        Ok(Self {
            cod_respuesta: string_field(src, "codRespuesta")?,
            error: present_field(src, "error")
                .map(|v| StatusResponseError::from_source(codec::model_source(v).as_ref()))
                .transpose()?,
            arc_cdr: string_field(src, "arcCdr")?,
            ind_cdr_generado: string_field(src, "indCdrGenerado")?,
        })
    }
}

impl Model for StatusResponse {
    fn model_name(&self) -> &'static str {
        "StatusResponse"
    }
}

impl ToJsonValue for StatusResponse {
    fn to_json_value(&self) -> Json {
        let mut f = Vec::new();
        push_str(&mut f, "codRespuesta", self.cod_respuesta.as_ref());
        if let Some(e) = &self.error {
            f.push(("error".to_owned(), e.to_json_value()));
        }
        push_str(&mut f, "arcCdr", self.arc_cdr.as_ref());
        push_str(&mut f, "indCdrGenerado", self.ind_cdr_generado.as_ref());
        Json::Object(f)
    }
}

/// Error del envío consultado (`StatusResponse_error`).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct StatusResponseError {
    /// `numError`.
    pub num_error: Option<String>,
    /// `desError`.
    pub des_error: Option<String>,
}

impl fmt::Debug for StatusResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StatusResponseError(<contenido oculto>)")
    }
}

impl StatusResponseError {
    pub(crate) fn from_source(src: Option<&Map<String, Value>>) -> Result<Self, DecodeError> {
        Ok(Self {
            num_error: string_field(src, "numError")?,
            des_error: string_field(src, "desError")?,
        })
    }
}

impl Model for StatusResponseError {
    fn model_name(&self) -> &'static str {
        "StatusResponse_error"
    }
}

impl ToJsonValue for StatusResponseError {
    fn to_json_value(&self) -> Json {
        let mut f = Vec::new();
        push_str(&mut f, "numError", self.num_error.as_ref());
        push_str(&mut f, "desError", self.des_error.as_ref());
        Json::Object(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn src(v: &Value) -> Option<Map<String, Value>> {
        codec::model_source(v)
    }

    #[test]
    fn lenient_reading() {
        let s = StatusResponse::from_source(
            src(&json!({"codRespuesta": 0, "error": "{\"numError\":7}"})).as_ref(),
        )
        .unwrap();
        assert_eq!(s.cod_respuesta.as_deref(), Some("0"));
        assert_eq!(s.error.unwrap().num_error.as_deref(), Some("7"));
        let t = ApiToken::from_source(
            src(&json!({"expires_in": "3600s", "access_token": true})).as_ref(),
        )
        .unwrap();
        assert_eq!(
            (t.expires_in, t.access_token.as_deref()),
            (Some(3600), Some("1"))
        );
        let e = CpeErrorValidation::from_source(src(&json!({"errors": {}})).as_ref()).unwrap_err();
        assert_eq!(e.class, "InvalidArgumentException");
        let r = CpeResponse::from_source(src(&json!({"fecRecepcion": ""})).as_ref()).unwrap_err();
        assert_eq!(r.message, "non-nullable fec_recepcion cannot be null");
        assert!(format!("{t:?}").contains("<oculto>"));
    }
}
