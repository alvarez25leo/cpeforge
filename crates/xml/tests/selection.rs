//! Selección de plantilla, contrato `Builder` y errores, con la API pública.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use cpeforge_core::model::company::Company;
use cpeforge_core::model::despatch::{AdditionalDoc, Despatch};
use cpeforge_core::model::sale::{BaseSale, Invoice, Note};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_core::ports::Builder;
use cpeforge_core::{CoreError, Zoned};
use cpeforge_xml::{
    Autoescape, DespatchBuilder, InvoiceBuilder, NoteBuilder, RenderOptions, SummaryBuilder,
    VoidedBuilder, XmlError,
};

fn fixed() -> RenderOptions {
    RenderOptions {
        now: Some("2024-01-01T12:00:00Z".parse().unwrap()),
        ..RenderOptions::default()
    }
}

fn invoice(version: Option<&str>) -> Invoice {
    Invoice {
        base: BaseSale {
            ubl_version: version.map(str::to_owned),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                ..Company::default()
            }),
            ..BaseSale::default()
        },
        ..Invoice::default()
    }
}

fn root(xml: &str) -> String {
    roxmltree::Document::parse(xml)
        .unwrap()
        .root_element()
        .tag_name()
        .name()
        .to_owned()
}

#[test]
fn invoice_version_selects_template() {
    let builder = InvoiceBuilder::new(fixed());
    assert!(
        builder
            .render(&invoice(Some("2.0")))
            .unwrap()
            .contains("<cbc:UBLVersionID>2.0</cbc:UBLVersionID>")
    );
    assert!(
        builder
            .render(&invoice(Some("2.1")))
            .unwrap()
            .contains("<cbc:UBLVersionID>2.1</cbc:UBLVersionID>")
    );
    assert!(
        InvoiceBuilder::default()
            .render(&Invoice::default())
            .unwrap()
            .contains("<cbc:UBLVersionID>2.0</cbc:UBLVersionID>")
    );
    for (version, template) in [
        (Some("2.2"), "invoice2.2.xml.twig"),
        (None, "invoice.xml.twig"),
        (Some(""), "invoice.xml.twig"),
        (Some("2.1 "), "invoice2.1 .xml.twig"),
    ] {
        assert_eq!(
            builder.render(&invoice(version)),
            Err(XmlError::TemplateNotFound {
                template: template.into()
            })
        );
    }
}

#[test]
fn note_type_selects_credit_or_debit() {
    let builder = NoteBuilder::new(fixed());
    let note = |tipo: Option<&str>, version: &str| Note {
        base: BaseSale {
            tipo_doc: tipo.map(str::to_owned),
            ubl_version: Some(version.into()),
            ..BaseSale::default()
        },
        ..Note::default()
    };
    for version in ["2.0", "2.1"] {
        assert_eq!(
            root(&builder.render(&note(Some("07"), version)).unwrap()),
            "CreditNote"
        );
        for other in [Some("08"), Some("01"), Some("7"), Some("07 "), None] {
            assert_eq!(
                root(&builder.render(&note(other, version)).unwrap()),
                "DebitNote",
                "{other:?}"
            );
        }
    }
    assert_eq!(
        builder.render(&note(Some("07"), "3")),
        Err(XmlError::TemplateNotFound {
            template: "notacr3.xml.twig".into()
        })
    );
    assert_eq!(
        builder.render(&note(Some("08"), "3")),
        Err(XmlError::TemplateNotFound {
            template: "notadb3.xml.twig".into()
        })
    );
}

#[test]
fn despatch_2022_only_on_exact_version() {
    let builder = DespatchBuilder::new(fixed());
    let customization = |version: Option<&str>| {
        let xml = builder
            .render(&Despatch {
                version: version.map(str::to_owned),
                ..Despatch::default()
            })
            .unwrap();
        xml.split("<cbc:CustomizationID>")
            .nth(1)
            .unwrap()
            .split('<')
            .next()
            .unwrap()
            .to_owned()
    };
    assert_eq!(customization(Some("2022")), "2.0");
    for other in [None, Some("2022 "), Some("2023"), Some("1")] {
        assert_eq!(customization(other), "1.0", "{other:?}");
    }
}

#[test]
fn strict_variables_and_model_errors() {
    let strict = RenderOptions {
        strict_variables: true,
        ..fixed()
    };
    let err = InvoiceBuilder::new(strict.clone())
        .render(&Invoice {
            base: BaseSale {
                ubl_version: Some("2.1".into()),
                ..BaseSale::default()
            },
            ..Invoice::default()
        })
        .unwrap_err();
    assert_eq!(
        err,
        XmlError::NullAttribute {
            template: "invoice2.1.xml.twig",
            attribute: "ruc"
        }
    );
    assert_eq!(
        err.to_string(),
        "Impossible to access an attribute (\"ruc\") on a null variable in \"invoice2.1.xml.twig\""
    );

    let summary = SummaryBuilder::new(fixed())
        .render(&Summary::default())
        .unwrap_err();
    assert_eq!(
        summary,
        XmlError::Document {
            template: "summary.xml.twig",
            source: CoreError::MissingField {
                model: "Summary",
                field: "fecResumen"
            }
        }
    );
    assert!(std::error::Error::source(&summary).is_some());

    let despatch = Despatch {
        version: Some("2022".into()),
        add_docs: Some(vec![AdditionalDoc::default()]),
        ..Despatch::default()
    };
    assert!(matches!(
        DespatchBuilder::new(fixed()).render(&despatch),
        Err(XmlError::Document {
            source: CoreError::MissingField { field: "nro", .. },
            ..
        })
    ));
}

#[test]
fn builder_contract_and_voided_prefixes() {
    let date: Zoned = "2024-02-01T03:00:00+00:00[UTC]".parse().unwrap();
    let voided = Voided {
        fec_comunicacion: Some(date),
        correlativo: Some("1".into()),
        ..Voided::default()
    };
    let builder = VoidedBuilder::new(fixed());
    let ra = Builder::<Voided>::build(&builder, &voided)
        .unwrap()
        .unwrap();
    let rr = Builder::<Reversion>::build(&builder, &Reversion { voided })
        .unwrap()
        .unwrap();
    assert!(ra.contains("<cbc:ID>RA-20240131-1</cbc:ID>"));
    assert!(rr.contains("<cbc:ID>RR-20240131-1</cbc:ID>"));
    let generic: &dyn Builder<Invoice, Error = XmlError> = &InvoiceBuilder::new(fixed());
    assert!(generic.build(&invoice(Some("2.1"))).unwrap().is_some());
}

#[test]
fn options_presets() {
    assert_eq!(RenderOptions::default().autoescape, Autoescape::Html);
    assert!(!RenderOptions::default().strict_variables);
    assert_eq!(RenderOptions::cpeforge().autoescape, Autoescape::Disabled);
    let strict = RenderOptions::strict();
    assert!(strict.strict_variables && strict.autoescape == Autoescape::Disabled);
}

#[test]
fn null_date_uses_injected_clock_in_peru_time() {
    let xml = InvoiceBuilder::new(fixed())
        .render(&invoice(Some("2.1")))
        .unwrap();
    // 2024-01-01T12:00:00Z es 07:00:00 en Lima.
    assert!(xml.contains(
        "<cbc:IssueDate>2024-01-01</cbc:IssueDate><cbc:IssueTime>07:00:00</cbc:IssueTime>"
    ));
}
