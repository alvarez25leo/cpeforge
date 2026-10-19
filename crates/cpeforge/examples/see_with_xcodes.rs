//! Conexión local del catálogo de xcodes con See; no envía solicitudes.

use cpeforge::See;
use cpeforge_gre_api::{
    HttpRequest, HttpResponse, HttpTransport, TransportError, TransportErrorKind,
};
use cpeforge_xcodes::XmlErrorCodeProvider;
use std::sync::Arc;

#[derive(Clone)]
struct NoNetwork;

impl HttpTransport for NoNetwork {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        Err(TransportError {
            kind: TransportErrorKind::Other,
            message: "red deshabilitada en el ejemplo".into(),
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let codes = XmlErrorCodeProvider::new()?;
    let mut see = See::new(NoNetwork);
    see.set_code_provider(Some(Arc::new(codes)));
    println!(
        "Proveedor de {} códigos conectado a See",
        codes.iter().len()
    );
    Ok(())
}
