//! Flujo mínimo de un consumidor (lo que hace `cpeforge-ws`): token, envío y
//! consulta contra un transporte simulado en memoria. No usa la red ni
//! credenciales reales.

use std::sync::Mutex;

use cpeforge_gre_api::{
    AuthApi, Configuration, CpeApi, CpeDocument, CpeDocumentArchivo, HttpRequest, HttpResponse,
    HttpTransport, TokenRequest, TransportError,
};

/// Responde según la ruta y registra lo enviado.
#[derive(Default)]
struct Simulado {
    vistos: Mutex<Vec<String>>,
}

impl HttpTransport for Simulado {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        if let Ok(mut v) = self.vistos.lock() {
            v.push(format!("{} {}", request.method.as_str(), request.url));
        }
        let body = if request.url.contains("/oauth2/token/") {
            r#"{"access_token":"token-ficticio","token_type":"Bearer","expires_in":3600}"#
        } else if request.url.contains("/envios/") {
            r#"{"codRespuesta":"98"}"#
        } else {
            r#"{"numTicket":"00000000-0000-0000-0000-000000000000","fecRecepcion":"2024-01-02T10:20:30-05:00"}"#
        };
        Ok(HttpResponse {
            status: 200,
            reason: "OK".to_owned(),
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            body: body.as_bytes().to_vec(),
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transporte = Simulado::default();
    let auth = AuthApi::new(&transporte, Configuration::auth());
    let token = auth
        .get_token(&TokenRequest::password_grant(
            "cliente-ficticio",
            "secreto-ficticio",
            "20000000001USUARIO",
            "clave-ficticia",
        ))
        .await?;
    println!("token recibido ({token:?})");

    let cpe = CpeApi::new(
        &transporte,
        Configuration::cpe(token.access_token.unwrap_or_default()),
    );
    let documento = CpeDocument {
        archivo: Some(CpeDocumentArchivo {
            nom_archivo: Some("20000000001-09-T001-1.zip".to_owned()),
            // El ZIP, su Base64 y su SHA-256 los prepara el llamador (`cpeforge-ws`).
            arc_gre_zip: Some("UEsDBBQAAAAIAA==".to_owned()),
            hash_zip: Some("0".repeat(64)),
        }),
    };
    let envio = cpe
        .enviar_cpe("20000000001-09-T001-1", Some(&documento))
        .await?;
    let ticket = envio.num_ticket.unwrap_or_default();
    let fecha = envio
        .fec_recepcion
        .map(|f| f.parse().map(|p| p.format_micro()));
    println!("ticket {ticket}, recepción {fecha:?}");
    let estado = cpe.consultar_envio(&ticket).await?;
    println!("codRespuesta {:?}", estado.cod_respuesta);
    for s in transporte
        .vistos
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default()
    {
        println!("  {s}");
    }
    Ok(())
}
