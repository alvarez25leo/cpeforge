//! Bytes que el refactor de rendimiento no puede cambiar: envelope SOAP y ZIP.
//!
//! Los valores se obtuvieron con la implementación anterior a las
//! optimizaciones (commit 873289f) y fijan la salida observable.

use std::sync::{Arc, Mutex};

use cpeforge_gre_api::{HttpRequest, HttpResponse, HttpTransport, TransportError};
use cpeforge_ws::WsError;
use cpeforge_ws::soap::{SoapClient, WsClient};
use cpeforge_ws::zip::{DosDateTime, ZipTime, compress_at};
use sha2::{Digest, Sha256};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<HttpRequest>>>);

impl HttpTransport for Capture {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        if let Ok(mut calls) = self.0.lock() {
            calls.push(request);
        }
        Ok(HttpResponse {
            status: 200,
            reason: "OK".into(),
            headers: vec![],
            body: Vec::new(),
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn invoice(lines: u32) -> Vec<u8> {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Invoice xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\">",
    );
    for i in 0..lines {
        xml.push_str(&format!(
            "<Line n=\"{i}\"><Amount>{}.{:02}</Amount><Text>Producto {i} {:x}</Text></Line>",
            i * 7919 % 100_000,
            i % 100,
            u64::from(i).wrapping_mul(0x9e37_79b9)
        ));
    }
    xml.push_str("</Invoice>");
    xml.into_bytes()
}

const ENVELOPE_SEND_BILL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><soap:Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\" xmlns:ns=\"http://service.sunat.gob.pe\" xmlns:wsse=\"http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd\"><soap:Header><wsse:Security><wsse:UsernameToken><wsse:Username>20000000001MODDATOS&amp;&lt;&gt;&quot;&apos;ñ</wsse:Username><wsse:Password Type=\"http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordText\">clave&amp;</wsse:Password></wsse:UsernameToken></wsse:Security></soap:Header><soap:Body><ns:sendBill><fileName>20000000001-01-F001-1.zip</fileName><contentFile>AAECA/z9/v8=</contentFile></ns:sendBill></soap:Body></soap:Envelope>";

#[tokio::test]
async fn soap_envelopes_are_byte_identical() -> Result<(), WsError> {
    let capture = Capture::default();
    let client = SoapClient::new(
        capture.clone(),
        "http://127.0.0.1:1/billService",
        "20000000001MODDATOS&<>\"'ñ",
        "clave&",
    );
    client
        .send_bill(
            "20000000001-01-F001-1.zip",
            &[0, 1, 2, 3, 252, 253, 254, 255],
        )
        .await?;
    client
        .send_summary("20000000001-RC-20261017-1.zip", b"zip")
        .await?;
    // Las consultas fallan al recibir un cuerpo vacío, pero la solicitud ya salió.
    let _ = client.get_status("123&<").await;
    let _ = client.consult(true, "20000000001", "01", "F001", -7).await;
    let request = client.request("getStatus", &[("ticket", "a<b>&\"'".to_owned())])?;
    let calls = capture.0.lock().map(|c| c.clone()).unwrap_or_default();
    assert_eq!(calls.len(), 4);
    assert_eq!(
        calls
            .first()
            .map(|c| String::from_utf8_lossy(&c.body).into_owned()),
        Some(ENVELOPE_SEND_BILL.to_owned())
    );
    let digests: Vec<String> = calls
        .iter()
        .chain(std::iter::once(&request))
        .map(|c| hex(&c.body))
        .collect();
    assert_eq!(
        digests,
        [
            "2a6eb1cf7343f87b8bf027bc195ae82a139638bbefb7668cb97015dae2dc25a0",
            "7d2d9d5130334e31d20786521d8fd61db2f59f14a82c80b5b7155e57ed0afe9e",
            "8d77944341913e4c363f80ad6420b31c42d8e3db4093f96117641153a31d7487",
            "0ffb54d04fa7a36b95b1d1fa5d8a2ded7464f0fc27da68788a1c61c8ece3fd2c",
            "8ebf282bbccdc6dee250dbad0ece7edbd30037a98061bf7240b878af67dc8a9a",
        ]
    );
    let headers: Vec<_> = calls.iter().map(|c| c.headers.clone()).collect();
    assert_eq!(
        headers.first(),
        Some(&vec![
            (
                "Content-Type".to_owned(),
                "text/xml; charset=utf-8".to_owned()
            ),
            ("SOAPAction".to_owned(), "\"urn:sendBill\"".to_owned()),
        ])
    );
    assert_eq!(
        calls
            .get(3)
            .and_then(|c| c.headers.get(1))
            .map(|h| h.1.as_str()),
        Some("\"urn:getStatusCdr\"")
    );
    Ok(())
}

#[test]
fn zip_bytes_are_identical() -> Result<(), WsError> {
    let time = ZipTime::Fixed(DosDateTime::new(2026, 10, 17, 12, 0, 0).unwrap_or(DosDateTime::MIN));
    let cases: [(&str, Vec<u8>); 4] = [
        ("vacio.xml", Vec::new()),
        ("20000000001-01-F001-Ñ.xml", "<x>á</x>".as_bytes().to_vec()),
        ("20000000001-01-F001-1.xml", invoice(800)),
        ("20000000001-01-F001-2.xml", invoice(60_000)),
    ];
    let digests = cases
        .iter()
        .map(|(name, content)| compress_at(name, content, time).map(|zip| (zip.len(), hex(&zip))))
        .collect::<Result<Vec<_>, _>>()?;
    let expected = [
        (
            118,
            "8635ab7dc21074fddb82f65f16fcc92ef118781dedb9eff7760c178bd5b3f121",
        ),
        (
            161,
            "4a590f2ae9629d2bc5a33f4d0e394bc44e44b323468622ea180e9134c6674dd7",
        ),
        (
            14353,
            "f1d4806c6f055634f44fdb46e6537541178ef2674548f3be7fe9c937e18e7173",
        ),
        (
            1_076_647,
            "e6c02734c24a4072146926e152bbcf6f956a587698255b4f92bbc707bd7494b5",
        ),
    ];
    let expected: Vec<_> = expected
        .iter()
        .map(|(n, h)| (*n, (*h).to_owned()))
        .collect();
    assert_eq!(digests, expected);
    Ok(())
}
