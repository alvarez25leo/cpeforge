//! Comunicación de baja y resumen de reversiones.

use cpeforge_core::model::company::Company;
use cpeforge_core::model::voided::{Reversion, Voided, VoidedDetail};
use cpeforge_core::{CoreError, Document};

use crate::convert::{DateContext, explode_part, loose_eq};
use crate::error::XmlParserError;
use crate::reader::Lookup;

super::parser!(VoidedParser, VoidedDocument, "Voided` o `Reversion");

/// Resultado de [`VoidedParser`]: `Voided` si el identificador empieza por
/// `RA` y `Reversion` en cualquier otro caso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoidedDocument {
    /// Comunicación de baja (`RA-…`).
    Voided(Voided),
    /// Resumen de reversiones (`RR-…` u otro prefijo).
    Reversion(Reversion),
}

impl VoidedDocument {
    /// Datos comunes de la comunicación.
    #[must_use]
    pub fn voided(&self) -> &Voided {
        match self {
            Self::Voided(voided) => voided,
            Self::Reversion(reversion) => &reversion.voided,
        }
    }

    /// Nombre corto del modelo (`Voided` o `Reversion`).
    #[must_use]
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Voided(_) => "Voided",
            Self::Reversion(_) => "Reversion",
        }
    }
}

impl Document for VoidedDocument {
    fn name(&self) -> Result<String, CoreError> {
        match self {
            Self::Voided(voided) => voided.name(),
            Self::Reversion(reversion) => reversion.name(),
        }
    }
}

fn parse(xml: &Lookup<'_>, dates: &DateContext) -> Result<VoidedDocument, XmlParserError> {
    let root = Some(xml.root());
    let id = xml.value("cbc:ID", root)?;
    let is_voided = loose_eq(&explode_part(&id, 0).unwrap_or_default(), "RA");
    let fec_generacion = dates.parse(&xml.value("cbc:ReferenceDate", root)?)?;
    let fec_comunicacion = dates.parse(&xml.value("cbc:IssueDate", root)?)?;
    let node = xml.node("cac:AccountingSupplierParty", root)?;
    let company = Company {
        ruc: Some(xml.value("cbc:CustomerAssignedAccountID", node)?),
        nombre_comercial: Some(xml.value("cac:Party/cac:PartyName/cbc:Name", node)?),
        razon_social: Some(xml.value("cac:Party/cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        ..Company::default()
    };
    let mut details = Vec::new();
    for line in xml.nodes("sac:VoidedDocumentsLine", root)? {
        let at = Some(line);
        details.push(VoidedDetail {
            tipo_doc: Some(xml.value("cbc:DocumentTypeCode", at)?),
            serie: Some(xml.value("sac:DocumentSerialID", at)?),
            correlativo: Some(xml.value("sac:DocumentNumberID", at)?),
            des_motivo_baja: Some(xml.value("sac:VoidReasonDescription", at)?),
        });
    }
    let voided = Voided {
        correlativo: explode_part(&id, 2),
        fec_generacion: Some(fec_generacion),
        fec_comunicacion: Some(fec_comunicacion),
        company: Some(company),
        details: Some(details),
    };
    Ok(if is_voided {
        VoidedDocument::Voided(voided)
    } else {
        VoidedDocument::Reversion(Reversion { voided })
    })
}
