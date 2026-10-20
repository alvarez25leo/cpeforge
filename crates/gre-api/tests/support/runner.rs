//! Ejecutor Rust de los casos: misma salida JSON que
//! el oráculo externo (`gre_api`).

use std::time::Duration;

use cpeforge_gre_api::config::{self, HOST_SETTINGS};
use cpeforge_gre_api::{
    ApiError, ApiResponse, ApiToken, AuthApi, Configuration, CpeApi, CpeDocument,
    CpeDocumentArchivo, CpeError, CpeErrorValidation, CpeResponse, ErrorObject, Headers,
    HttpRequest, Model, ReqwestTransport, StatusResponse, StatusResponseError, TokenRequest,
};
use serde_json::{Map, Value, json};

fn s(v: &Value) -> Option<String> {
    match v {
        Value::Object(m) if m.contains_key("repeat") => {
            let r = &m["repeat"];
            Some(
                r["unit"]
                    .as_str()
                    .unwrap_or("")
                    .repeat(usize::try_from(r["count"].as_u64().unwrap_or(0)).unwrap_or(0)),
            )
        }
        other => other.as_str().map(str::to_owned),
    }
}

fn bytes_out(b: &[u8]) -> Value {
    match std::str::from_utf8(b) {
        Ok(t) => json!(t),
        Err(_) => json!({"b64": b64(b)}),
    }
}

fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            out.push(if i <= c.len() {
                char::from(T[((n >> (18 - 6 * i)) & 63) as usize])
            } else {
                '='
            });
        }
    }
    out
}

fn headers_out(h: &Headers) -> Value {
    Value::Array(
        h.iter()
            .map(|(k, v)| json!([k.to_ascii_lowercase(), v]))
            .collect(),
    )
}

fn obj(name: &str, fields: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    m.insert("_model".into(), json!(name));
    for (k, v) in fields {
        m.insert(k.into(), v);
    }
    Value::Object(m)
}

pub(crate) trait Dump {
    fn dump(&self) -> Value;
}

impl Dump for ApiToken {
    fn dump(&self) -> Value {
        obj(
            self.model_name(),
            vec![
                ("access_token", json!(self.access_token)),
                ("token_type", json!(self.token_type)),
                ("expires_in", json!(self.expires_in)),
            ],
        )
    }
}

impl Dump for CpeResponse {
    fn dump(&self) -> Value {
        let fecha = self.fec_recepcion.as_ref().map(|f| match f.parse() {
            Ok(p) => json!(p.format_micro()),
            Err(e) => json!({"_unparsed": f.raw(), "_motivo": e.to_string()}),
        });
        obj(
            self.model_name(),
            vec![
                ("numTicket", json!(self.num_ticket)),
                ("fecRecepcion", json!(fecha)),
            ],
        )
    }
}

impl Dump for StatusResponseError {
    fn dump(&self) -> Value {
        obj(
            self.model_name(),
            vec![
                ("numError", json!(self.num_error)),
                ("desError", json!(self.des_error)),
            ],
        )
    }
}

impl Dump for StatusResponse {
    fn dump(&self) -> Value {
        obj(
            self.model_name(),
            vec![
                ("codRespuesta", json!(self.cod_respuesta)),
                ("error", self.error.as_ref().map_or(Value::Null, Dump::dump)),
                ("arcCdr", json!(self.arc_cdr)),
                ("indCdrGenerado", json!(self.ind_cdr_generado)),
            ],
        )
    }
}

impl Dump for CpeError {
    fn dump(&self) -> Value {
        obj(
            self.model_name(),
            vec![("cod", json!(self.cod)), ("msg", json!(self.msg))],
        )
    }
}

impl Dump for CpeErrorValidation {
    fn dump(&self) -> Value {
        let errors = self.errors.as_ref().map(|l| {
            Value::Array(
                l.iter()
                    .map(|e| e.as_ref().map_or(Value::Null, Dump::dump))
                    .collect(),
            )
        });
        obj(
            self.model_name(),
            vec![
                ("cod", json!(self.cod)),
                ("msg", json!(self.msg)),
                ("errors", errors.unwrap_or(Value::Null)),
            ],
        )
    }
}

fn data<T: Dump>(r: Result<T, ApiError>) -> Result<Value, ApiError> {
    r.map(|d| json!({"data": d.dump()}))
}

fn info<T: Dump>(r: Result<ApiResponse<T>, ApiError>) -> Result<Value, ApiError> {
    r.map(|i| json!({"data": i.data.as_ref().map_or(Value::Null, Dump::dump), "status": i.status, "headers": headers_out(&i.headers)}))
}

fn request_out(r: &HttpRequest) -> Value {
    json!({
        "method": r.method.as_str(),
        "url": r.url,
        "headers": r.headers.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        "body": bytes_out(&r.body),
    })
}

pub(crate) fn error_out(e: &ApiError) -> Value {
    let mut out = json!({"class": e.error_class(), "message": e.message(), "code": e.code()});
    if matches!(e, ApiError::Http { .. } | ApiError::Transport { .. }) {
        out["headers"] = e.response_headers().map_or(Value::Null, headers_out);
        out["body"] = e.response_body().map_or(Value::Null, bytes_out);
        out["object"] = match e.response_object() {
            Some(ErrorObject::Server(x)) => x.dump(),
            Some(ErrorObject::Validation(x)) => x.dump(),
            Some(_) | None => Value::Null,
        };
    }
    out
}

fn config_for(c: &Value, base: &str) -> Configuration {
    let cfg = &c["config"];
    let mut config = Configuration::default();
    match cfg["host"].as_str() {
        Some("mock") => {
            config = config.with_host(format!("{base}/c/{}", c["id"].as_str().unwrap_or("")))
        }
        Some(h) => config = config.with_host(h),
        None => {}
    }
    if let Some(t) = cfg.get("access_token") {
        config = config.with_access_token(t.as_str().unwrap_or(""));
    }
    if let Some(ua) = cfg.get("user_agent") {
        config = config.with_user_agent(ua.as_str().unwrap_or(""));
    }
    config
}

fn document(d: &Value) -> Option<CpeDocument> {
    if d.is_null() {
        return None;
    }
    let archivo = d
        .get("archivo")
        .filter(|a| !a.is_null())
        .map(|a| CpeDocumentArchivo {
            nom_archivo: s(&a["nom_archivo"]),
            arc_gre_zip: s(&a["arc_gre_zip"]),
            hash_zip: s(&a["hash_zip"]),
        });
    Some(CpeDocument { archivo })
}

fn config_dump() -> Value {
    let cfg = Configuration::default();
    let hosts: Vec<Value> = [0i64, 1, 2, -1]
        .iter()
        .map(|i| match config::host_from_settings(*i) {
            Ok(h) => json!({"index": i, "host": h}),
            Err(e) => json!({"index": i, "error": {"class": "InvalidArgumentException", "message": e.to_string(), "code": 0}}),
        })
        .collect();
    json!({
        "host": cfg.host(),
        "user_agent": cfg.user_agent(),
        "access_token": cfg.access_token().map_or("", |t| t.expose()),
        "host_settings": HOST_SETTINGS.iter().map(|h| json!({"url": h.url, "description": h.description})).collect::<Vec<_>>(),
        "host_from_settings": hosts,
        "cpe_api_host": cfg.host(),
    })
}

/// Ejecuta un caso; `None` si no aplica a Rust.
pub(crate) async fn run_case(c: &Value, base: &str) -> Option<Result<Value, ApiError>> {
    if c["rust"] == "no_aplica" {
        return None;
    }
    let variant = c["variant"].as_str().unwrap_or("");
    if c["operation"] == "config" {
        return Some(Ok(config_dump()));
    }
    if variant != "request" {
        assert!(
            matches!(
                c["config"]["host"].as_str(),
                Some("mock" | "http://127.0.0.1:1")
            ),
            "los casos de red solo pueden usar destinos locales"
        );
    }
    let mut builder = ReqwestTransport::builder();
    if let Some(t) = c["client"]["timeout"].as_u64() {
        builder = builder.timeout(Duration::from_secs(t));
    }
    let transport = builder.build().unwrap();
    let config = config_for(c, base);
    let ct = c.get("content_type").and_then(Value::as_str);
    let a = &c["args"];
    let http_info = variant.contains("http_info");
    Some(match c["operation"].as_str().unwrap_or("") {
        "getToken" => {
            let api = AuthApi::new(transport, config);
            let req = TokenRequest {
                grant_type: s(&a["grant_type"]).unwrap_or_default(),
                scope: s(&a["scope"]).unwrap_or_default(),
                client_id: s(&a["client_id"]).unwrap_or_default(),
                client_secret: s(&a["client_secret"]).unwrap_or_default().into(),
                username: s(&a["username"]).unwrap_or_default(),
                password: s(&a["password"]).unwrap_or_default().into(),
            };
            if variant == "request" {
                Ok(request_out(&api.get_token_request(&req, ct)))
            } else if variant == "async_http_info" {
                info(api.get_token_async_with_http_info(&req, ct).await)
            } else if variant == "async" {
                data(api.get_token_async(&req).await)
            } else if http_info {
                info(api.get_token_with_http_info(&req, ct).await)
            } else {
                data(api.get_token(&req).await)
            }
        }
        "enviarCpe" => {
            let api = CpeApi::new(transport, config);
            let filename = s(&a["filename"]).unwrap_or_default();
            let doc = document(&a["document"]);
            if variant == "request" {
                Ok(request_out(&api.enviar_cpe_request(
                    &filename,
                    doc.as_ref(),
                    ct,
                )))
            } else if variant == "async_http_info" {
                info(
                    api.enviar_cpe_async_with_http_info(&filename, doc.as_ref(), ct)
                        .await,
                )
            } else if variant == "async" {
                data(api.enviar_cpe_async(&filename, doc.as_ref()).await)
            } else if http_info {
                info(
                    api.enviar_cpe_with_http_info(&filename, doc.as_ref(), ct)
                        .await,
                )
            } else {
                data(api.enviar_cpe(&filename, doc.as_ref()).await)
            }
        }
        _ => {
            let api = CpeApi::new(transport, config);
            let ticket = s(&a["ticket"]).unwrap_or_default();
            if variant == "request" {
                Ok(request_out(&api.consultar_envio_request(&ticket, ct)))
            } else if variant == "async_http_info" {
                info(api.consultar_envio_async_with_http_info(&ticket, ct).await)
            } else if variant == "async" {
                data(api.consultar_envio_async(&ticket).await)
            } else if http_info {
                info(api.consultar_envio_with_http_info(&ticket, ct).await)
            } else {
                data(api.consultar_envio(&ticket).await)
            }
        }
    })
}
