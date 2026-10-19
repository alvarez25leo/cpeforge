//! Ejemplo sintético: ZIP/CDR y solicitud SOAP construida sin enviarla.

use cpeforge_gre_api::ReqwestTransport;
use cpeforge_ws::cdr::read_cdr_zip;
use cpeforge_ws::soap::SoapClient;
use cpeforge_ws::zip::compress;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xml = r#"<ApplicationResponse xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"><cac:DocumentResponse><cac:Response><cbc:ResponseCode>0</cbc:ResponseCode></cac:Response></cac:DocumentResponse></ApplicationResponse>"#;
    let zip = compress("R-FICTICIO.xml", xml.as_bytes())?;
    let cdr = read_cdr_zip(&zip)?;
    assert_eq!(cdr.code.as_deref(), Some("0"));

    let client = SoapClient::new(
        ReqwestTransport::new()?,
        "http://127.0.0.1:1/soap",
        "USUARIO-FICTICIO",
        "CLAVE-FICTICIA",
    );
    let request = client.request("getStatus", &[("ticket", "TICKET-FICTICIO".into())])?;
    println!(
        "CDR {} y {} SOAP preparados localmente",
        cdr.code.unwrap_or_default(),
        request.method.as_str()
    );
    Ok(())
}
