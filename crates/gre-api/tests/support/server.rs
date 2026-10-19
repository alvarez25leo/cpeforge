//! Servidor HTTP local en proceso para la comparación offline: responde lo
//! guionado en los casos igual que `oracle/gre_api/mock_server.py` (mismas
//! frases de estado y cabeceras) y registra las solicitudes. Solo escucha en
//! 127.0.0.1.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Registro de solicitudes por caso.
pub(crate) type Records = Arc<Mutex<HashMap<String, Vec<Value>>>>;

/// Frases de `http.server` de Python 3.9 para los códigos usados.
fn python_reason(status: u64) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        300 => "Multiple Choices",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        418 => "I'm a Teapot",
        422 => "Unprocessable Entity",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "",
    }
}

pub(crate) struct Server {
    pub(crate) port: u16,
    pub(crate) records: Records,
}

pub(crate) async fn start(cases: HashMap<String, Value>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let records: Records = Arc::new(Mutex::new(HashMap::new()));
    let cases = Arc::new(cases);
    let rec = records.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let cases = cases.clone();
            let rec = rec.clone();
            tokio::spawn(async move {
                let _ = handle(stream, &cases, &rec, port).await;
            });
        }
    });
    Server { port, records }
}

async fn handle(
    mut stream: TcpStream,
    cases: &HashMap<String, Value>,
    records: &Records,
    port: u16,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| {
            l.split_once(':')
                .map(|(k, v)| (k.to_owned(), v.trim().to_owned()))
        })
        .collect();
    let length: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let Some(rest) = target.strip_prefix("/c/") else {
        return Ok(());
    };
    let (id, api_path) = rest.split_once('/').map_or((rest, ""), |(a, b)| (a, b));
    let api_path = format!("/{api_path}");
    let Some(case) = cases.get(id) else {
        return Ok(());
    };
    let mut script = case.get("response").cloned().unwrap_or(json!({}));
    let is_final = api_path.ends_with("/final") && script.get("final").is_some();
    if is_final {
        script = script["final"].clone();
    }
    let body_value = match String::from_utf8(body.clone()) {
        Ok(s) => json!(s),
        Err(_) => json!({"b64": base64_encode(&body)}),
    };
    let record = json!({
        "method": method,
        "path": api_path,
        "headers": headers.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        "body": body_value,
    });
    records
        .lock()
        .unwrap()
        .entry(id.to_owned())
        .or_default()
        .push(record);
    if script["action"] == "close" {
        return Ok(());
    }
    if let Some(d) = script["delay"].as_u64() {
        tokio::time::sleep(Duration::from_secs(d)).await;
    }
    let status = script["status"].as_u64().unwrap_or(200);
    let reason = script["reason"]
        .as_str()
        .unwrap_or(python_reason(status))
        .to_owned();
    let resp_body = script["body"].as_str().unwrap_or("").as_bytes().to_vec();
    let mut out = format!("HTTP/1.1 {status} {reason}\r\n");
    for h in script["headers"].as_array().into_iter().flatten() {
        let k = h[0].as_str().unwrap_or("");
        let mut v = h[1].as_str().unwrap_or("").to_owned();
        if k == "Location" && v.starts_with('@') {
            let host = if v.starts_with("@same/") {
                "127.0.0.1"
            } else {
                "localhost"
            };
            let tail = v.split_once('/').map(|(_, t)| t).unwrap_or("");
            v = format!("http://{host}:{port}/c/{id}/{tail}");
        }
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        resp_body.len()
    ));
    stream.write_all(out.as_bytes()).await?;
    stream.write_all(&resp_body).await?;
    stream.shutdown().await
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= c.len() {
                out.push(char::from(T[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}
