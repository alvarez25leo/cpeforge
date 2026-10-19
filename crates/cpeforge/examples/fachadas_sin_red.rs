//! Ejemplo reproducible de ambas fachadas con transporte simulado.

use cpeforge::{Api, DocumentKind, See};
use cpeforge_gre_api::{
    Configuration, HttpRequest, HttpResponse, HttpTransport, Method, TransportError,
};
use cpeforge_ws::api::{ApiFactory, InMemoryStore, SystemClock};

#[derive(Clone)]
struct Local;
impl HttpTransport for Local {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = if request.url.ends_with("/oauth2/token/") {
            br#"{"access_token":"token-ficticio","expires_in":3600}"#.to_vec()
        } else if request.method == Method::Post && request.url.contains("/contribuyente/") {
            br#"{"numTicket":"T-SINTETICO"}"#.to_vec()
        } else {
            br#"<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"><soap:Body><sendSummaryResponse xmlns="http://service.sunat.gob.pe"><ticket>T-SINTETICO</ticket></sendSummaryResponse></soap:Body></soap:Envelope>"#.to_vec()
        };
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body,
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut see = See::new(Local);
    see.set_service("http://127.0.0.1:9/soap").set_clave_sol(
        "20000000001",
        "TEST",
        "clave-ficticia",
    );
    let soap = see
        .send_xml(
            DocumentKind::Summary,
            "20000000001-RC-20240131-1",
            b"<xml-sintetico/>",
        )
        .await?;
    println!("SOAP: {soap:?}");

    let factory = ApiFactory::new(
        Local,
        Configuration::auth().with_host("http://127.0.0.1:9/auth"),
        "http://127.0.0.1:9/cpe",
        InMemoryStore::default(),
        SystemClock,
    );
    let mut api = Api::new(factory);
    api.set_api_credentials("cliente-ficticio", "secreto-ficticio")
        .set_clave_sol("20000000001", "TEST", "clave-ficticia");
    let gre = api
        .send_xml("20000000001-09-T001-1", b"<xml-sintetico/>")
        .await?;
    println!("GRE: {:?}", gre.ticket);
    Ok(())
}
