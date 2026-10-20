//! Contrato de la API pública: el motivo de rechazo implementa
//! `std::error::Error` con `Send + Sync + 'static` (sirve con `?`,
//! `Box<dyn Error>` y `anyhow`).

fn error<T: std::error::Error + Send + Sync + 'static>() {}
#[test]
fn public_types_are_thread_safe_errors_are_standard() {
    error::<cpeforge_xml_limits::MarkupRejection>();
}
