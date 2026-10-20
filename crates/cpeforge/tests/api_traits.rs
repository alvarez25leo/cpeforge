//! Contrato de la API pública: los tipos principales se pueden compartir
//! entre hilos (Tokio) y los errores implementan `std::error::Error` con
//! `Send + Sync + 'static` (sirven con `?`, `Box<dyn Error>` y `anyhow`).

fn send_sync<T: Send + Sync>() {}
fn error<T: std::error::Error + Send + Sync + 'static>() {}
#[test]
fn public_types_are_thread_safe_errors_are_standard() {
    error::<cpeforge::CpeForgeError>();
    error::<cpeforge_core::CoreError>();
    error::<cpeforge_xml::XmlError>();
    error::<cpeforge_xmldsig::Error>();
    error::<cpeforge_xmldsig::XmlError>();
    error::<cpeforge_gre_api::ApiError>();
    error::<cpeforge_gre_api::TransportError>();
    error::<cpeforge_gre_api::DateTimeError>();
    error::<cpeforge_ws::WsError>();
    send_sync::<cpeforge_xmldsig::dom::Document>();
    send_sync::<cpeforge_xmldsig::key::XmlSecurityKey>();
    send_sync::<cpeforge_xmldsig::dsig::XmlSecurityDsig>();
    send_sync::<cpeforge_xmldsig::sunat::SignedXml>();
    send_sync::<cpeforge_gre_api::ReqwestTransport>();
    send_sync::<cpeforge_ws::api::InMemoryStore>();
    send_sync::<cpeforge::See<cpeforge_gre_api::ReqwestTransport>>();
}
