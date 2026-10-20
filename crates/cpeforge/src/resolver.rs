//! Selección tipada de builders y servicios.

use cpeforge_core::Document;
use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{Invoice, Note};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_xml::{
    DespatchBuilder, InvoiceBuilder, NoteBuilder, PerceptionBuilder, RenderOptions,
    RetentionBuilder, SummaryBuilder, VoidedBuilder,
};

use crate::CpeForgeError;

/// Una referencia a cada modelo que tiene builder en `cpeforge-xml`.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum DocumentRef<'a> {
    /// Factura o boleta.
    Invoice(&'a Invoice),
    /// Nota de crédito o débito.
    Note(&'a Note),
    /// Guía.
    Despatch(&'a Despatch),
    /// Resumen RC.
    Summary(&'a Summary),
    /// Baja RA.
    Voided(&'a Voided),
    /// Reversión RR.
    Reversion(&'a Reversion),
    /// Percepción.
    Perception(&'a Perception),
    /// Retención.
    Retention(&'a Retention),
}

impl std::fmt::Debug for DocumentRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DocumentRef").field(&self.kind()).finish()
    }
}

impl DocumentRef<'_> {
    /// Tipo de documento.
    #[must_use]
    pub fn kind(self) -> DocumentKind {
        match self {
            Self::Invoice(_) => DocumentKind::Invoice,
            Self::Note(_) => DocumentKind::Note,
            Self::Despatch(_) => DocumentKind::Despatch,
            Self::Summary(_) => DocumentKind::Summary,
            Self::Voided(_) => DocumentKind::Voided,
            Self::Reversion(_) => DocumentKind::Reversion,
            Self::Perception(_) => DocumentKind::Perception,
            Self::Retention(_) => DocumentKind::Retention,
        }
    }

    /// Equivalente de `DocumentInterface::getName()`.
    pub fn name(self) -> Result<String, CpeForgeError> {
        let value = match self {
            Self::Invoice(d) => d.name(),
            Self::Note(d) => d.name(),
            Self::Despatch(d) => d.name(),
            Self::Summary(d) => d.name(),
            Self::Voided(d) => d.name(),
            Self::Reversion(d) => d.name(),
            Self::Perception(d) => d.name(),
            Self::Retention(d) => d.name(),
        };
        value.map_err(Into::into)
    }
}

/// Tipos de documento admitidos por `XmlBuilderResolver`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DocumentKind {
    /// Factura o boleta.
    Invoice,
    /// Nota de crédito o débito.
    Note,
    /// Guía de remisión.
    Despatch,
    /// Resumen RC.
    Summary,
    /// Comunicación de baja RA.
    Voided,
    /// Reversión RR.
    Reversion,
    /// Percepción.
    Perception,
    /// Retención.
    Retention,
}

impl DocumentKind {
    /// Convierte el nombre de tipo que devuelve `ws::cdr::resolve_type`
    /// (`CPEForge\Model\…`); cualquier otro es [`CpeForgeError::UnsupportedDocument`].
    pub fn from_class_name(class: &str) -> Result<Self, CpeForgeError> {
        match class {
            "CPEForge\\Model\\Sale\\Invoice" => Ok(Self::Invoice),
            "CPEForge\\Model\\Sale\\Note" => Ok(Self::Note),
            "CPEForge\\Model\\Despatch\\Despatch" => Ok(Self::Despatch),
            "CPEForge\\Model\\Summary\\Summary" => Ok(Self::Summary),
            "CPEForge\\Model\\Voided\\Voided" => Ok(Self::Voided),
            "CPEForge\\Model\\Voided\\Reversion" => Ok(Self::Reversion),
            "CPEForge\\Model\\Perception\\Perception" => Ok(Self::Perception),
            "CPEForge\\Model\\Retention\\Retention" => Ok(Self::Retention),
            _ => Err(CpeForgeError::UnsupportedDocument),
        }
    }
}

/// Rama del `WsSenderResolver`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SenderKind {
    /// Envío con CDR sincrónico.
    Bill,
    /// Envío con ticket.
    Summary,
}

/// Equivalente tipado de `WsSenderResolver::find`.
#[must_use]
pub fn sender_kind(kind: DocumentKind) -> SenderKind {
    match kind {
        DocumentKind::Summary | DocumentKind::Voided | DocumentKind::Reversion => {
            SenderKind::Summary
        }
        _ => SenderKind::Bill,
    }
}

/// Construye XML sin firma usando los builders existentes.
pub trait XmlBuilder: Send + Sync {
    /// XML UBL anterior a la firma.
    fn build(&self, document: DocumentRef<'_>) -> Result<String, CpeForgeError>;
}

/// Selector de builder, incluida la correspondencia `Reversion → VoidedBuilder`.
#[derive(Debug, Clone)]
pub struct XmlBuilderResolver {
    options: RenderOptions,
}

impl XmlBuilderResolver {
    /// Resolver con las opciones de render indicadas.
    #[must_use]
    pub fn new(options: RenderOptions) -> Self {
        Self { options }
    }

    /// Obtiene un builder para un tipo conocido.
    #[must_use]
    pub fn find(&self, kind: DocumentKind) -> ResolvedBuilder {
        ResolvedBuilder {
            kind,
            options: self.options.clone(),
        }
    }
}

/// Builder ya resuelto.
#[derive(Debug, Clone)]
pub struct ResolvedBuilder {
    kind: DocumentKind,
    options: RenderOptions,
}

impl XmlBuilder for ResolvedBuilder {
    fn build(&self, document: DocumentRef<'_>) -> Result<String, CpeForgeError> {
        if document.kind() != self.kind {
            return Err(CpeForgeError::UnsupportedDocument);
        }
        let opts = self.options.clone();
        match document {
            DocumentRef::Invoice(d) => InvoiceBuilder::new(opts).render(d),
            DocumentRef::Note(d) => NoteBuilder::new(opts).render(d),
            DocumentRef::Despatch(d) => DespatchBuilder::new(opts).render(d),
            DocumentRef::Summary(d) => SummaryBuilder::new(opts).render(d),
            DocumentRef::Voided(d) => VoidedBuilder::new(opts).render(d),
            DocumentRef::Reversion(d) => VoidedBuilder::new(opts).render_reversion(d),
            DocumentRef::Perception(d) => PerceptionBuilder::new(opts).render(d),
            DocumentRef::Retention(d) => RetentionBuilder::new(opts).render(d),
        }
        .map_err(Into::into)
    }
}
