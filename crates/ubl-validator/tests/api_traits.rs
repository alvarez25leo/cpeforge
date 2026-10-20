//! Contrato de la API pública: los tipos principales se pueden compartir
//! entre hilos (Tokio) y los errores implementan `std::error::Error` con
//! `Send + Sync + 'static` (sirven con `?`, `Box<dyn Error>` y `anyhow`).

fn send_sync<T: Send + Sync>() {}
fn error<T: std::error::Error + Send + Sync + 'static>() {}
#[test]
fn public_types_are_thread_safe_errors_are_standard() {
    error::<cpeforge_ubl_validator::InfrastructureError>();
    send_sync::<cpeforge_ubl_validator::UblValidator>();
}
