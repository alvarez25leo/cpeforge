//! Selección de plantilla por tipo de documento.
//!
//! Es una comodidad aparte: [`crate::HtmlReport`] empieza siempre con
//! `invoice.html.twig` y el llamador decide si usa el resolver.

use crate::error::ReportError;
use crate::html::{ReportDocument, class_name};

/// Elige la plantilla de un documento.
pub trait TemplateResolver {
    /// Nombre de la plantilla.
    ///
    /// # Errors
    ///
    /// [`ReportError::InvalidDocument`] si el tipo no tiene plantilla.
    fn template(&self, document: &dyn ReportDocument) -> Result<String, ReportError>;
}

/// Resolver por tipo de documento:
///
/// | Documento | Plantilla |
/// | --- | --- |
/// | `Invoice`, `Note` | `invoice.html.twig` |
/// | `Retention` | `retention.html.twig` |
/// | `Perception` | `perception.html.twig` |
/// | `Despatch` | `despatch.html.twig` |
/// | `Summary` | `summary.html.twig` |
/// | `Voided`, `Reversion` | `voided.html.twig` |
/// | `PurchaseSettlement` | `settlement.html.twig` |
///
/// Otro tipo (`BaseSale`, `Receipt`) produce [`ReportError::InvalidDocument`]
/// con el identificador de clase del modelo
/// (`Not found template for CPEForge\Model\Sale\Receipt`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultTemplateResolver;

impl TemplateResolver for DefaultTemplateResolver {
    fn template(&self, document: &dyn ReportDocument) -> Result<String, ReportError> {
        let class = class_name(document);
        let name = match class.rsplit('\\').next().unwrap_or_default() {
            "Invoice" | "Note" => "invoice",
            "Retention" => "retention",
            "Perception" => "perception",
            "Despatch" => "despatch",
            "Summary" => "summary",
            "Voided" | "Reversion" => "voided",
            "PurchaseSettlement" => "settlement",
            _ => {
                return Err(ReportError::InvalidDocument {
                    class: class.to_owned(),
                });
            }
        };
        Ok(format!("{name}.html.twig"))
    }
}
