//! Contrato de la API pública: el validador y las hojas compiladas se
//! comparten entre hilos; `XmlDocument` (un documento de libxml2) se puede
//! mover a otro hilo pero no compartir. Los errores implementan
//! `std::error::Error` con `Send + Sync + 'static`.

fn send_sync<T: Send + Sync>() {}
fn send<T: Send>() {}
fn error<T: std::error::Error + Send + Sync + 'static>() {}
#[test]
fn public_types_are_thread_safe_errors_are_standard() {
    send_sync::<cpeforge_cpe_validator::CpeValidator>();
    send_sync::<cpeforge_cpe_validator::Stylesheet>();
    send::<cpeforge_cpe_validator::XmlDocument>();
    error::<cpeforge_cpe_validator::CpeValidatorError>();
}
