//! Regresión de la revisión de seguridad: un parámetro de ruta `.` o `..` no
//! llega al transporte (la URL se normalizaría y el token o las credenciales
//! irían a otra ruta del host). Sin red.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cpeforge_gre_api::{
    ApiError, AuthApi, Configuration, CpeApi, HttpRequest, HttpResponse, HttpTransport,
    TokenRequest, TransportError,
};

#[derive(Clone, Default)]
struct Counting(Arc<AtomicUsize>);

impl HttpTransport for Counting {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: br#"{"codRespuesta":"98"}"#.to_vec(),
        })
    }
}

fn is_invalid_request(error: &ApiError) -> bool {
    matches!(error, ApiError::InvalidRequest { .. })
        && error.code() == 0
        && error.error_class() == "InvalidArgumentException"
        && !format!("{error:?}").contains("parámetro")
}

#[tokio::test]
async fn dot_path_parameters_never_reach_the_transport() {
    let transport = Counting::default();
    let cpe = CpeApi::new(
        transport.clone(),
        Configuration::cpe("TOKEN-FICTICIO").with_host("http://127.0.0.1:1/v1"),
    );
    for value in [".", ".."] {
        assert!(is_invalid_request(
            &cpe.enviar_cpe(value, None).await.unwrap_err()
        ));
        assert!(is_invalid_request(
            &cpe.consultar_envio(value).await.unwrap_err()
        ));
    }
    let auth = AuthApi::new(
        transport.clone(),
        Configuration::auth().with_host("http://127.0.0.1:1/v1"),
    );
    let request = TokenRequest::password_grant("..", "SECRETO-FICTICIO", "USUARIO", "CLAVE");
    assert!(is_invalid_request(
        &auth.get_token(&request).await.unwrap_err()
    ));
    assert_eq!(transport.0.load(Ordering::SeqCst), 0);

    // `%2e%2e` se codifica como `%252e%252e`: no es un segmento de punto.
    assert!(cpe.consultar_envio("%2e%2e").await.is_ok());
    assert!(cpe.consultar_envio("...").await.is_ok());
    assert_eq!(transport.0.load(Ordering::SeqCst), 2);
}

#[cfg(feature = "reqwest")]
#[tokio::test]
async fn reqwest_transport_rejects_dot_segments_without_connecting() {
    use cpeforge_gre_api::{Method, ReqwestTransport, TransportErrorKind};

    let transport = ReqwestTransport::new().unwrap();
    for url in [
        "http://127.0.0.1:1/v1/contribuyente/gem/comprobantes/..",
        "http://127.0.0.1:1/v1/a/./b",
        "http://127.0.0.1:1/v1/a/%2E%2e/b?x=1",
    ] {
        let error = transport
            .send(HttpRequest {
                method: Method::Get,
                url: url.to_owned(),
                headers: vec![],
                body: vec![],
            })
            .await
            .unwrap_err();
        assert_eq!(error.kind, TransportErrorKind::InvalidUrl, "{url}");
    }
}
