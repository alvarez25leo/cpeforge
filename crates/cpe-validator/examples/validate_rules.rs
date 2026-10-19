//! Valida dos facturas sintéticas con una regla XSL propia (no es una regla
//! SUNAT: el paquete no distribuye esas hojas). La regla exige
//! `UBLVersionID = 2.1` usando `XsltFunctions::matches`, como la hoja de
//! prueba de la referencia.

use std::error::Error;
use std::fs;

use cpeforge_cpe_validator::{CpeValidator, MATCHES_HANDLER, XslPathResolver};

const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";

fn main() -> Result<(), Box<dyn Error>> {
    let rules = std::env::temp_dir().join(format!("cpeforge-cpe-example-{}", std::process::id()));
    fs::create_dir_all(rules.join("2.X"))?;
    let rule = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:cbc="{CBC}" xmlns:php="http://php.net/xsl">
  <xsl:param name="nombreArchivoEnviado"/>
  <xsl:template match="/*">
    <xsl:variable name="version" select="cbc:UBLVersionID"/>
    <xsl:if test="not(php:function('{MATCHES_HANDLER}', '^(2.1)$', string($version)) = 1)">
      <xsl:message terminate="yes">
        <xsl:value-of select="concat('1|2074|UBLVersionID - La versión del UBL no es correcta|Invoice/UBLVersionID|', $version)"/>
      </xsl:message>
    </xsl:if>
  </xsl:template>
</xsl:stylesheet>"#
    );
    let rule_path = rules.join(XslPathResolver::relative_path(
        cpeforge_cpe_validator::DocumentType::Factura,
    ));
    fs::write(&rule_path, rule)?;

    let validator = CpeValidator::new(&rules);
    for version in ["2.1", "2.0"] {
        let xml = format!(
            r#"<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2" xmlns:cbc="{CBC}"><cbc:UBLVersionID>{version}</cbc:UBLVersionID></Invoice>"#
        );
        let errors = validator.validate_xml("20000000001-01-F001-1.xml", xml)?;
        println!("UBL {version}: {} error(es)", errors.len());
        for error in &errors {
            println!(
                "  [{}] {} {} ({} = {})",
                error.level.map_or("", |l| l.code()),
                error.code.as_deref().unwrap_or_default(),
                error.message.as_deref().unwrap_or_default(),
                error.node_path.as_deref().unwrap_or_default(),
                error.node_value.as_deref().unwrap_or_default(),
            );
        }
        let expected = usize::from(version != "2.1");
        if errors.len() != expected {
            return Err(format!("se esperaban {expected} errores para UBL {version}").into());
        }
    }
    fs::remove_dir_all(&rules)?;
    Ok(())
}
