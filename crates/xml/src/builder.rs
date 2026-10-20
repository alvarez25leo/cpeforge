//! Builders por familia de documento.
//!
//! Cada builder implementa [`cpeforge_core::ports::Builder`] para su
//! modelo. `build` devuelve el XML sin firma; el nombre de la
//! plantilla se elige con las mismas reglas, y una versión sin plantilla es un
//! error [`XmlError::TemplateNotFound`].

use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{Invoice, Note};
use cpeforge_core::model::settlement::PurchaseSettlement;
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_core::ports::Builder;

use crate::error::XmlError;
use crate::options::RenderOptions;
use crate::templates::{despatch, invoice20, invoice21, note, retention, settlement, summary};

macro_rules! builder {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default)]
        pub struct $name {
            options: RenderOptions,
        }

        impl $name {
            /// Builder con las opciones de render indicadas.
            #[must_use]
            pub fn new(options: RenderOptions) -> Self {
                Self { options }
            }

            /// Opciones de render.
            #[must_use]
            pub fn options(&self) -> &RenderOptions {
                &self.options
            }
        }
    };
}

builder!(
    /// `InvoiceBuilder`: factura y boleta. Plantilla
    /// `invoice{ublVersion}.xml.twig` (`2.0` o `2.1`).
    InvoiceBuilder
);
builder!(
    /// `NoteBuilder`: notas de crédito (`tipoDoc` exactamente `07`) y de
    /// débito (cualquier otro valor), `notacr`/`notadb` + `ublVersion`.
    NoteBuilder
);
builder!(
    /// `DespatchBuilder`: `despatch2022.xml.twig` si `version` es exactamente
    /// `2022`; `despatch.xml.twig` en cualquier otro caso.
    DespatchBuilder
);
builder!(
    /// `SummaryBuilder`: resumen diario (`summary.xml.twig`).
    SummaryBuilder
);
builder!(
    /// `VoidedBuilder`: comunicación de baja y resumen de reversiones
    /// (`voided.xml.twig`); el prefijo `RA`/`RR` lo da el modelo.
    VoidedBuilder
);
builder!(
    /// `PerceptionBuilder`: comprobante de percepción.
    PerceptionBuilder
);
builder!(
    /// `RetentionBuilder`: comprobante de retención.
    RetentionBuilder
);
builder!(
    /// `SettlementBuilder`: liquidación de compra (`SelfBilledInvoice`, tipo
    /// `04`).
    SettlementBuilder
);

impl InvoiceBuilder {
    /// XML de una factura o boleta.
    ///
    /// # Errors
    ///
    /// [`XmlError::TemplateNotFound`] si `ublVersion` no es `2.0` ni `2.1`, y
    /// los errores de render de [`XmlError`].
    pub fn render(&self, invoice: &Invoice) -> Result<String, XmlError> {
        match invoice.base.ubl_version.as_deref() {
            Some("2.0") => invoice20::render(invoice, &self.options),
            Some("2.1") => invoice21::render(invoice, &self.options),
            other => Err(XmlError::TemplateNotFound {
                template: format!("invoice{}.xml.twig", other.unwrap_or_default()),
            }),
        }
    }
}

impl NoteBuilder {
    /// XML de una nota de crédito o débito.
    ///
    /// # Errors
    ///
    /// [`XmlError::TemplateNotFound`] si `ublVersion` no es `2.0` ni `2.1`, y
    /// los errores de render de [`XmlError`].
    pub fn render(&self, doc: &Note) -> Result<String, XmlError> {
        let kind = if doc.base.tipo_doc.as_deref() == Some("07") {
            note::Kind::Credit
        } else {
            note::Kind::Debit
        };
        let version = doc.base.ubl_version.as_deref().unwrap_or_default();
        match kind.template(version) {
            Some(template) if version == "2.1" => {
                note::render_21(doc, kind, template, &self.options)
            }
            Some(template) => note::render_20(doc, kind, template, &self.options),
            None => {
                let prefix = if kind == note::Kind::Credit {
                    "notacr"
                } else {
                    "notadb"
                };
                Err(XmlError::TemplateNotFound {
                    template: format!("{prefix}{version}.xml.twig"),
                })
            }
        }
    }
}

impl DespatchBuilder {
    /// XML de una guía de remisión.
    ///
    /// # Errors
    ///
    /// Los errores de render de [`XmlError`].
    pub fn render(&self, doc: &Despatch) -> Result<String, XmlError> {
        if doc.version.as_deref() == Some("2022") {
            despatch::render_2022(doc, &self.options)
        } else {
            despatch::render(doc, &self.options)
        }
    }
}

impl SummaryBuilder {
    /// XML de un resumen diario.
    ///
    /// # Errors
    ///
    /// [`XmlError::Document`] si falta `fecResumen` (el identificador `RC`
    /// necesita la fecha) y los errores de render de [`XmlError`].
    pub fn render(&self, doc: &Summary) -> Result<String, XmlError> {
        summary::render_summary(doc, &self.options)
    }
}

impl VoidedBuilder {
    /// XML de una comunicación de baja (`RA`).
    ///
    /// # Errors
    ///
    /// [`XmlError::Document`] si falta `fecComunicacion` y los errores de
    /// render de [`XmlError`].
    pub fn render(&self, doc: &Voided) -> Result<String, XmlError> {
        summary::render_voided(
            summary::VoidedView {
                xml_id: doc.xml_id(),
                fec_generacion: doc.fec_generacion.as_ref(),
                fec_comunicacion: doc.fec_comunicacion.as_ref(),
                company: doc.company.as_ref(),
                details: doc.details.as_ref(),
            },
            &self.options,
        )
    }

    /// XML de un resumen de reversiones (`RR`).
    ///
    /// # Errors
    ///
    /// Los mismos que [`VoidedBuilder::render`].
    pub fn render_reversion(&self, doc: &Reversion) -> Result<String, XmlError> {
        let voided = &doc.voided;
        summary::render_voided(
            summary::VoidedView {
                xml_id: doc.xml_id(),
                fec_generacion: voided.fec_generacion.as_ref(),
                fec_comunicacion: voided.fec_comunicacion.as_ref(),
                company: voided.company.as_ref(),
                details: voided.details.as_ref(),
            },
            &self.options,
        )
    }
}

impl PerceptionBuilder {
    /// XML de un comprobante de percepción.
    ///
    /// # Errors
    ///
    /// Los errores de render de [`XmlError`].
    pub fn render(&self, doc: &Perception) -> Result<String, XmlError> {
        retention::render_perception(doc, &self.options)
    }
}

impl RetentionBuilder {
    /// XML de un comprobante de retención.
    ///
    /// # Errors
    ///
    /// Los errores de render de [`XmlError`].
    pub fn render(&self, doc: &Retention) -> Result<String, XmlError> {
        retention::render_retention(doc, &self.options)
    }
}

impl SettlementBuilder {
    /// XML de una liquidación de compra.
    ///
    /// # Errors
    ///
    /// [`XmlError::Document`] si falta el emisor, el vendedor o el lugar de
    /// la operación, y los errores de render de [`XmlError`].
    pub fn render(&self, doc: &PurchaseSettlement) -> Result<String, XmlError> {
        settlement::render(doc, &self.options)
    }
}

macro_rules! port {
    ($builder:ty, $doc:ty, $method:ident) => {
        /// Contrato `BuilderInterface`: siempre devuelve `Some(xml)` o un error.
        impl Builder<$doc> for $builder {
            type Error = XmlError;

            fn build(&self, document: &$doc) -> Result<Option<String>, XmlError> {
                self.$method(document).map(Some)
            }
        }
    };
}

port!(InvoiceBuilder, Invoice, render);
port!(NoteBuilder, Note, render);
port!(DespatchBuilder, Despatch, render);
port!(SummaryBuilder, Summary, render);
port!(VoidedBuilder, Voided, render);
port!(VoidedBuilder, Reversion, render_reversion);
port!(PerceptionBuilder, Perception, render);
port!(RetentionBuilder, Retention, render);
port!(SettlementBuilder, PurchaseSettlement, render);
