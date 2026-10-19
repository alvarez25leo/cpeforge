//! Verificación viva contra el servidor Python independiente en loopback.

use cpeforge::{Api, DocumentKind, See, SendResult};
use cpeforge_gre_api::{Configuration, ReqwestTransport};
use cpeforge_ws::api::{ApiFactory, InMemoryStore, SystemClock};

#[tokio::test]
#[ignore = "usar scripts/cpeforge_local_contract.sh con egress bloqueado"]
async fn soap_and_gre_local_contract() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::var("CPEFORGE_LOCAL_URL")?;
    let transport = ReqwestTransport::new()?;
    let mut see = See::new(transport.clone());
    see.set_service(format!("{base}/soap"))
        .set_clave_sol("20000000001", "TEST", "password.test");
    let sent = see
        .send_xml(
            DocumentKind::Summary,
            "20000000001-RC-20240131-1",
            b"<caller-xml/>",
        )
        .await?;
    let SendResult::Summary(summary) = sent else {
        return Err("se esperaba SummaryResult SOAP".into());
    };
    assert_eq!(summary.ticket.as_deref(), Some("T-123"));
    assert_eq!(see.get_status("T-123").await?.code.as_deref(), Some("98"));

    let factory = ApiFactory::new(
        transport,
        Configuration::auth().with_host(format!("{base}/auth")),
        format!("{base}/cpe"),
        InMemoryStore::default(),
        SystemClock,
    );
    let mut api = Api::new(factory);
    api.set_api_credentials("client.test", "secret.test")
        .set_clave_sol("20000000001", "TEST", "password.test");
    assert_eq!(
        api.send_xml("raw", b"<caller-xml/>")
            .await?
            .ticket
            .as_deref(),
        Some("T-123")
    );
    assert_eq!(api.get_status("T-123").await?.code.as_deref(), Some("98"));
    Ok(())
}
