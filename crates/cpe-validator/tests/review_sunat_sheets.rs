//! Hojas con la estructura de las publicadas por SUNAT («Archivo XSL – 2.1»,
//! 2022-09-06): incluyen `local:///commons/…` de IBM DataPower, que SUNAT no
//! publica, y usan `dp:` y EXSLT `regexp:`. Se probaron las 15 hojas reales:
//! todas se analizan y se detienen en el `include`, sin red ni salida del
//! directorio de reglas. Esta hoja sintética reproduce esa estructura sin
//! copiar contenido de SUNAT.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::fs;

use cpeforge_cpe_validator::{AccessKind, CpeValidatorError, Limits, Stylesheet};

const SHEET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:regexp="http://exslt.org/regular-expressions"
    xmlns:dp="http://www.datapower.com/extensions"
    xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"
    extension-element-prefixes="dp" exclude-result-prefixes="dp">
  <xsl:include href="local:///commons/error/validate_utils.xsl" dp:ignore-multiple="yes"/>
  <xsl:template match="/*">
    <xsl:if test="not(regexp:match(cbc:UBLVersionID, '^(2.1)$'))">
      <xsl:message terminate="yes">1|2074|UBLVersionID</xsl:message>
    </xsl:if>
  </xsl:template>
</xsl:stylesheet>"#;

#[test]
fn datapower_local_includes_are_rejected_without_io() {
    let dir = std::env::temp_dir().join(format!("cpeforge-sunat-sheet-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ValidaExprRegFactura-2.0.1.xsl");
    fs::write(&path, SHEET).unwrap();

    let error = Stylesheet::from_file(&path, Some(&dir), &Limits::default())
        .err()
        .unwrap();
    match error {
        CpeValidatorError::AccessDenied { kind, resource, .. } => {
            assert_eq!(kind, AccessKind::ReadNetwork);
            assert_eq!(resource, "local:///commons/error/validate_utils.xsl");
        }
        other => panic!("{other:?}"),
    }
    fs::remove_dir_all(&dir).unwrap();
}
