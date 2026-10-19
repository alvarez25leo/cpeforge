//! Uso del crate como dependencia externa, sin serde: construcción de cada
//! familia de modelos, nombres de documento y contratos implementados por
//! dobles de prueba como lo harían los crates `xml`, `ws`, `report`,
//! `validator` y `xml-parser`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error as _;
use std::str::FromStr;

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::despatch::{AdditionalDoc, Despatch, Direction, Shipment, Vehicle};
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::response::{
    BaseResult, BillResult, CdrResponse, ResponseError, ServiceResult, StatusCdrResult,
    StatusResult, SummaryResult,
};
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{
    BaseSale, Invoice, Legend, Note, PaymentTerms, Receipt, SaleDetail,
};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided, VoidedDetail};
use cpeforge_core::ports::{
    Builder, DocumentParser, DocumentValidator, ErrorCodeProvider, Factory, Report,
};
use cpeforge_core::service::{BasicToken, InvalidServiceResponse, Sender, TokenStore};
use cpeforge_core::time::{PERU_TIME_ZONE, to_peru_time};
use cpeforge_core::{CoreError, Decimal, Document, Zoned};

fn company() -> Company {
    Company {
        ruc: Some("20000000001".into()),
        razon_social: Some("EMPRESA SINTETICA".into()),
        ..Company::default()
    }
}

fn amount(text: &str) -> Decimal {
    Decimal::from_str(text).unwrap_or_default()
}

fn lima(text: &str) -> Zoned {
    Zoned::from_str(text).unwrap_or_else(|err| panic!("fecha de prueba inválida {text}: {err}"))
}

fn invoice() -> Invoice {
    Invoice {
        base: BaseSale {
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("1".into()),
            fecha_emision: Some(lima("2024-01-31T23:30:00-05:00[America/Lima]")),
            company: Some(company()),
            client: Some(Client {
                tipo_doc: Some("6".into()),
                num_doc: Some("20000000002".into()),
                ..Client::default()
            }),
            tipo_moneda: Some("PEN".into()),
            mto_igv: Some(amount("18.00")),
            mto_imp_venta: Some(amount("118.00")),
            details: Some(vec![SaleDetail {
                descripcion: Some("ITEM".into()),
                mto_valor_unitario: Some(amount("100.00")),
                ..SaleDetail::default()
            }]),
            legends: Some(vec![Legend {
                code: Some("1000".into()),
                value: Some("SON CIENTO DIECIOCHO".into()),
            }]),
            forma_pago: Some(PaymentTerms::contado()),
            ..BaseSale::default()
        },
        tipo_operacion: Some("0101".into()),
        ..Invoice::default()
    }
}

#[test]
fn defaults_match_reference() {
    let address = Address::default();
    assert_eq!(address.codigo_pais.as_deref(), Some("PE"));
    assert_eq!(address.cod_local.as_deref(), Some("0000"));
    assert_eq!(BaseSale::default().ubl_version.as_deref(), Some("2.0"));
    assert_eq!(Invoice::default().base.ubl_version.as_deref(), Some("2.0"));
    assert_eq!(Note::default().base.ubl_version.as_deref(), Some("2.0"));
    assert_eq!(
        SaleDetail::default().factor_icbper,
        Some(Decimal::new(3, 1))
    );
    assert_eq!(Summary::default().moneda.as_deref(), Some("PEN"));
    assert_eq!(BaseResult::default().success, Some(false));
    assert_eq!(BillResult::default().success(), Some(false));
    assert_eq!(StatusCdrResult::default().success(), Some(false));
    let error = ResponseError::default();
    assert_eq!(
        (error.code.as_deref(), error.message.as_deref()),
        (Some(""), Some(""))
    );
    assert_eq!(
        ResponseError::with_code(Some("0100".into()))
            .message
            .as_deref(),
        Some("")
    );
    assert_eq!(
        PaymentTerms::credito(None, None).tipo.as_deref(),
        Some(PaymentTerms::CREDITO)
    );
}

#[test]
fn document_names_for_every_family() {
    let despatch = Despatch {
        tipo_doc: Some("09".into()),
        serie: Some("T001".into()),
        correlativo: Some("2".into()),
        company: Some(company()),
        envio: Some(Shipment {
            llegada: Some(Direction::new(Some("150101".into()), Some("AV. 1".into()))),
            vehiculo: Some(Vehicle {
                placa: Some("ABC-123".into()),
                secundarios: Some(vec![]),
                ..Vehicle::default()
            }),
            ..Shipment::default()
        }),
        ..Despatch::default()
    };
    let retention = Retention {
        serie: Some("R001".into()),
        correlativo: Some("3".into()),
        company: Some(company()),
        ..Retention::default()
    };
    let perception = Perception {
        serie: Some("P001".into()),
        correlativo: Some("4".into()),
        company: Some(company()),
        ..Perception::default()
    };
    let summary = Summary {
        correlativo: Some("001".into()),
        fec_resumen: Some(lima("2024-02-01T03:00:00+00:00[UTC]")),
        company: Some(company()),
        ..Summary::default()
    };
    let voided = Voided {
        correlativo: Some("002".into()),
        fec_comunicacion: Some(lima("2024-02-01T05:00:00+00:00[UTC]")),
        company: Some(company()),
        details: Some(vec![VoidedDetail::default()]),
        ..Voided::default()
    };
    let reversion = Reversion {
        voided: voided.clone(),
    };
    let receipt = Receipt {
        person: Some(Company {
            ruc: Some("10000000001".into()),
            ..Company::default()
        }),
        correlativo: Some("7".into()),
        ..Receipt::default()
    };
    let note = Note {
        base: BaseSale {
            tipo_doc: Some("07".into()),
            serie: Some("FC01".into()),
            correlativo: Some("5".into()),
            company: Some(company()),
            ..BaseSale::default()
        },
        ..Note::default()
    };

    let invoice = invoice();
    let documents: Vec<(&dyn Document, &str)> = vec![
        (&invoice, "20000000001-01-F001-1"),
        (&note, "20000000001-07-FC01-5"),
        (&despatch, "20000000001-09-T001-2"),
        (&retention, "20000000001-20-R001-3"),
        (&perception, "20000000001-40-P001-4"),
        (&summary, "20000000001-RC-20240131-001"),
        (&voided, "20000000001-RA-20240201-002"),
        (&reversion, "20000000001-RR-20240201-002"),
        (&receipt, "RHE100000000017"),
    ];
    for (document, expected) in documents {
        assert_eq!(document.name().as_deref(), Ok(expected));
    }
}

#[test]
fn missing_issuer_is_a_typed_error() {
    let err = Invoice::default().name().unwrap_err();
    assert_eq!(
        err,
        CoreError::MissingField {
            model: "Invoice",
            field: "company"
        }
    );
    assert_eq!(err.to_string(), "Invoice: falta el campo `company`");
    assert_eq!(
        Summary::default().xml_id().unwrap_err(),
        CoreError::MissingField {
            model: "Summary",
            field: "fecResumen"
        }
    );
    assert_eq!(
        AdditionalDoc::default().nro().unwrap_err(),
        CoreError::MissingField {
            model: "AdditionalDoc",
            field: "nro"
        }
    );
    assert_eq!(
        Receipt::default().name().unwrap_err(),
        CoreError::MissingField {
            model: "Receipt",
            field: "person"
        }
    );
}

#[test]
fn xml_id_does_not_change_the_original_date() {
    let date = lima("2024-02-01T03:00:00+00:00[UTC]");
    let summary = Summary {
        fec_resumen: Some(date.clone()),
        correlativo: Some("1".into()),
        ..Summary::default()
    };
    assert_eq!(summary.xml_id().as_deref(), Ok("RC-20240131-1"));
    assert_eq!(summary.fec_resumen, Some(date.clone()));
    let peru = to_peru_time(&date).unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(peru.time_zone().iana_name(), Some(PERU_TIME_ZONE));
    assert_eq!(peru.timestamp(), date.timestamp());
}

// ---------------------------------------------------------------------------
// Dobles de prueba de los contratos, con el uso que harán los crates consumidores.
// ---------------------------------------------------------------------------

/// Como un builder de `xml`: lee campos del modelo y devuelve texto.
struct InvoiceXml;

impl Builder<Invoice> for InvoiceXml {
    type Error = CoreError;

    fn build(&self, invoice: &Invoice) -> Result<Option<String>, CoreError> {
        let lines = invoice.base.details.as_deref().unwrap_or_default().len();
        Ok(Some(format!(
            "<Invoice id=\"{}\" lines=\"{lines}\"/>",
            invoice.name()?
        )))
    }
}

/// Builder que no produce contenido (`build()` devuelve `null` en la referencia).
struct EmptyXml;

impl Builder<Invoice> for EmptyXml {
    type Error = CoreError;

    fn build(&self, _: &Invoice) -> Result<Option<String>, CoreError> {
        Ok(None)
    }
}

/// Builder genérico sobre `dyn Document`.
struct NameOnly;

impl Builder for NameOnly {
    type Error = CoreError;

    fn build(&self, document: &dyn Document) -> Result<Option<String>, CoreError> {
        document.name().map(Some)
    }
}

/// Como un sender de `ws`: devuelve un CDR simulado sin red.
struct FakeSender;

impl Sender for FakeSender {
    type Output = BillResult;
    type Error = InvalidServiceResponse;

    fn send(
        &self,
        filename: Option<&str>,
        content: Option<&[u8]>,
    ) -> Result<Option<BillResult>, Self::Error> {
        let content = content
            .filter(|bytes| !bytes.is_empty())
            .ok_or_else(|| InvalidServiceResponse::new("sin contenido", 1))?;
        let cdr = CdrResponse {
            id: filename.map(str::to_owned),
            code: Some(if content.is_empty() {
                "2000".into()
            } else {
                "0".into()
            }),
            notes: Some(vec![]),
            ..CdrResponse::default()
        };
        let mut result = BillResult {
            cdr_zip: Some(content.to_vec()),
            cdr_response: Some(cdr),
            ..BillResult::default()
        };
        result.base.success = Some(true);
        Ok(Some(result))
    }
}

/// Como `cpeforge::FeFactory`: compone builder y sender.
struct FeFactory<B, S> {
    builder: B,
    sender: S,
}

#[derive(Debug)]
enum FactoryError {
    Build(CoreError),
    Send(InvalidServiceResponse),
}

impl<B: Builder<Invoice, Error = CoreError>, S: Sender> Factory<Invoice> for FeFactory<B, S>
where
    S: Sender<Error = InvalidServiceResponse>,
{
    type Output = S::Output;
    type Error = FactoryError;

    fn send(&self, invoice: &Invoice) -> Result<Option<S::Output>, FactoryError> {
        let xml = self
            .builder
            .build(invoice)
            .map_err(FactoryError::Build)?
            .unwrap_or_default();
        let name = invoice.name().map_err(FactoryError::Build)?;
        self.sender
            .send(Some(&name), Some(xml.as_bytes()))
            .map_err(FactoryError::Send)
    }
}

/// Como `xml-parser`: interpreta un nombre `RUC-TIPO-SERIE-CORRELATIVO`.
struct NameParser;

impl DocumentParser for NameParser {
    type Input = str;
    type Output = Invoice;
    type Error = String;

    fn parse(&self, value: &str) -> Result<Option<Invoice>, String> {
        let parts: Vec<&str> = value.split('-').collect();
        let [ruc, tipo, serie, correlativo] = parts.as_slice() else {
            return Ok(None);
        };
        Ok(Some(Invoice {
            base: BaseSale {
                company: Some(Company {
                    ruc: Some((*ruc).into()),
                    ..Company::default()
                }),
                tipo_doc: Some((*tipo).into()),
                serie: Some((*serie).into()),
                correlativo: Some((*correlativo).into()),
                ..BaseSale::default()
            },
            ..Invoice::default()
        }))
    }
}

/// Como `report`: usa parámetros de presentación.
struct TextReport;

impl Report for TextReport {
    type Parameters = HashMap<String, String>;
    type Error = CoreError;

    fn render(
        &self,
        document: &dyn Document,
        parameters: &Self::Parameters,
    ) -> Result<Option<String>, CoreError> {
        let header = parameters.get("header").map_or("", String::as_str);
        Ok(Some(format!("{header}{}", document.name()?)))
    }
}

/// Como `validator` + `xcodes`: catálogo ordenado de códigos.
struct Codes;

impl ErrorCodeProvider for Codes {
    type Error = std::convert::Infallible;

    fn all(&self) -> Result<Option<Vec<(String, String)>>, Self::Error> {
        Ok(Some(vec![
            ("0100".into(), "El sistema no puede responder".into()),
            ("2017".into(), "RUC inválido".into()),
        ]))
    }

    fn value(&self, code: Option<&str>) -> Result<Option<String>, Self::Error> {
        let all = self.all()?.unwrap_or_default();
        Ok(Some(
            all.into_iter()
                .find(|(key, _)| Some(key.as_str()) == code)
                .map(|(_, msg)| msg)
                .unwrap_or_default(),
        ))
    }
}

struct RucValidator<'a, P> {
    codes: &'a P,
}

impl<P: ErrorCodeProvider> DocumentValidator<Invoice> for RucValidator<'_, P> {
    type Output = Vec<String>;
    type Error = P::Error;

    fn validate(&self, invoice: &Invoice) -> Result<Option<Vec<String>>, P::Error> {
        let ruc = invoice
            .base
            .company
            .as_ref()
            .and_then(|c| c.ruc.as_deref())
            .unwrap_or_default();
        let mut violations = Vec::new();
        if ruc.len() != 11 {
            violations.push(self.codes.value(Some("2017"))?.unwrap_or_default());
        }
        Ok(Some(violations))
    }
}

/// Almacén en memoria como `ws\Api\InMemoryStore`.
#[derive(Default)]
struct InMemoryStore {
    tokens: RefCell<HashMap<String, BasicToken>>,
}

impl TokenStore for InMemoryStore {
    type Error = std::convert::Infallible;

    fn get(&self, id: Option<&str>) -> Result<Option<BasicToken>, Self::Error> {
        Ok(self.tokens.borrow().get(id.unwrap_or_default()).cloned())
    }

    fn set(&self, id: Option<&str>, token: BasicToken) -> Result<(), Self::Error> {
        self.tokens
            .borrow_mut()
            .insert(id.unwrap_or_default().to_owned(), token);
        Ok(())
    }
}

#[test]
fn contracts_compose_like_consumer_packages() {
    let factory = FeFactory {
        builder: InvoiceXml,
        sender: FakeSender,
    };
    let result = factory
        .send(&invoice())
        .unwrap_or_else(|err| panic!("{err:?}"))
        .unwrap_or_default();
    assert_eq!(result.success(), Some(true));
    assert!(result.error().is_none());
    let cdr = result.cdr_response.clone().unwrap_or_default();
    assert!(cdr.is_accepted());
    assert_eq!(cdr.id.as_deref(), Some("20000000001-01-F001-1"));
    assert_eq!(
        result.cdr_zip.as_deref(),
        Some(&b"<Invoice id=\"20000000001-01-F001-1\" lines=\"1\"/>"[..])
    );

    assert!(matches!(
        factory.send(&Invoice::default()),
        Err(FactoryError::Build(CoreError::MissingField { .. }))
    ));
    let empty = FeFactory {
        builder: EmptyXml,
        sender: FakeSender,
    };
    match empty.send(&invoice()) {
        Err(FactoryError::Send(err)) => {
            assert_eq!((err.message(), err.code()), ("sin contenido", 1))
        }
        other => panic!("se esperaba error del sender: {other:?}"),
    }

    let generic: &dyn Builder<Error = CoreError> = &NameOnly;
    assert_eq!(
        generic.build(&invoice()).ok().flatten().as_deref(),
        Some("20000000001-01-F001-1")
    );

    let parsed = NameParser
        .parse("20000000001-01-F001-1")
        .ok()
        .flatten()
        .unwrap_or_default();
    assert_eq!(parsed.name().as_deref(), Ok("20000000001-01-F001-1"));
    assert_eq!(NameParser.parse("x").ok().flatten(), None);

    let params = HashMap::from([("header".to_owned(), "Factura ".to_owned())]);
    assert_eq!(
        TextReport
            .render(&parsed, &params)
            .ok()
            .flatten()
            .as_deref(),
        Some("Factura 20000000001-01-F001-1")
    );

    let validator = RucValidator { codes: &Codes };
    let mut bad = invoice();
    bad.base.company = Some(Company {
        ruc: Some("123".into()),
        ..Company::default()
    });
    assert_eq!(
        validator.validate(&bad).ok().flatten(),
        Some(vec!["RUC inválido".to_owned()])
    );
    assert_eq!(validator.validate(&invoice()).ok().flatten(), Some(vec![]));

    let store = InMemoryStore::default();
    let token = BasicToken::new(
        Some("tok".into()),
        Some(lima("2024-01-01T12:00:00+00:00[UTC]")),
    );
    assert!(store.set(None, token.clone()).is_ok());
    assert_eq!(store.get(None).ok().flatten(), Some(token));
    assert_eq!(store.get(Some("otro")).ok().flatten(), None);
}

#[test]
fn service_results_share_base_fields() {
    let error = ResponseError::new(Some("0111".into()), Some("Sin perfil".into()));
    let mut status = StatusResult {
        code: Some("99".into()),
        ..StatusResult::default()
    };
    status.bill.base.error = Some(error.clone());
    assert_eq!(status.error(), Some(&error));
    assert_eq!(status.success(), Some(false));
    let summary = SummaryResult {
        ticket: Some("123".into()),
        ..SummaryResult::default()
    };
    assert_eq!(summary.success(), Some(false));
    let results: Vec<&dyn ServiceResult> = vec![&status, &summary];
    assert!(results.iter().all(|result| result.success() == Some(false)));
}

#[test]
fn invalid_service_response_keeps_message_code_and_cause() {
    let cause = std::io::Error::other("respuesta vacía");
    let err = InvalidServiceResponse::new("Respuesta inválida", 500).with_source(cause);
    assert_eq!(err.to_string(), "Respuesta inválida");
    assert_eq!(err.code(), 500);
    assert_eq!(
        err.source().map(ToString::to_string).as_deref(),
        Some("respuesta vacía")
    );
    let default = InvalidServiceResponse::default();
    assert_eq!((default.message(), default.code()), ("", 0));
    assert!(default.source().is_none());
}
