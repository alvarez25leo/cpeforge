//! Integración optativa del catálogo de xcodes con la traducción SOAP de See.
#![cfg(feature = "xcodes")]

use cpeforge::factory::SendResult;
use cpeforge::resolver::DocumentKind;
use cpeforge::{ErrorCodeProvider, See};
use cpeforge_gre_api::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use cpeforge_xcodes::XmlErrorCodeProvider;
use std::sync::Arc;

#[derive(Clone)]
struct FaultTransport {
    code: &'static str,
}

impl HttpTransport for FaultTransport {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        let xml = format!(
            "<soap:Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\"><soap:Body><soap:Fault><faultcode>soap:Client.{}</faultcode><faultstring>Original</faultstring></soap:Fault></soap:Body></soap:Envelope>",
            self.code
        );
        Ok(HttpResponse {
            status: 500,
            reason: "Fault".into(),
            // Como SUNAT: con `Content-Type` XML, ext/soap lee el fault aunque sea un 500.
            headers: vec![("Content-Type".into(), "text/xml; charset=utf-8".into())],
            body: xml.into_bytes(),
        })
    }
}

#[tokio::test]
async fn known_code_and_unknown_fallback() -> Result<(), Box<dyn std::error::Error>> {
    let provider = XmlErrorCodeProvider::new()?;
    assert_eq!(
        ErrorCodeProvider::get_value(&provider, "0100").as_deref(),
        Some(provider.get("0100"))
    );
    for (code, expected) in [("0100", provider.get("0100")), ("9999", "Original")] {
        let mut see = See::new(FaultTransport { code });
        see.set_code_provider(Some(Arc::new(provider)));
        let result = see
            .send_xml(DocumentKind::Invoice, "synthetic", b"<raw/>")
            .await?;
        let SendResult::Bill(bill) = result else {
            return Err("BillResult esperado".into());
        };
        let fault = bill.base.error.ok_or("fault esperado")?;
        assert_eq!(fault.message.as_deref(), Some(expected));
    }
    Ok(())
}
