//! Integración: XML generado por `cpeforge-xml` y leído por
//! `cpeforge-xml-parser`, validado con reglas sintéticas. Los esperados se
//! derivan del modelo, no de la salida del validador.

use std::error::Error;
use std::fs;
use std::str::FromStr;

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, PaymentTerms, SaleDetail};
use cpeforge_core::ports::DocumentParser;
use cpeforge_core::{Decimal, Document, Zoned};
use cpeforge_cpe_validator::{
    CpeValidator, DocumentType, ErrorLevel, MATCHES_HANDLER, TypeResolver, XmlDocument,
    XmlTypeResolver, XslPathResolver,
};
use cpeforge_xml::{InvoiceBuilder, RenderOptions};
use cpeforge_xml_parser::{InvoiceParser, ParseOptions};

type TestResult = Result<(), Box<dyn Error>>;

fn invoice(tipo_doc: &str, serie: &str, ubl: &str) -> Result<Invoice, Box<dyn Error>> {
    let amount = |text: &str| Decimal::from_str(text);
    Ok(Invoice {
        base: BaseSale {
            ubl_version: Some(ubl.into()),
            tipo_doc: Some(tipo_doc.into()),
            serie: Some(serie.into()),
            correlativo: Some("123".into()),
            fecha_emision: Some(Zoned::from_str("2024-01-31T10:30:00-05:00[America/Lima]")?),
            tipo_moneda: Some("PEN".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                razon_social: Some("EMPRESA SINTETICA S.A.C.".into()),
                address: Some(Address {
                    ubigueo: Some("150101".into()),
                    direccion: Some("AV. FICTICIA 123".into()),
                    ..Address::default()
                }),
                ..Company::default()
            }),
            client: Some(Client {
                tipo_doc: Some("6".into()),
                num_doc: Some("20000000002".into()),
                rzn_social: Some("CLIENTE".into()),
                ..Client::default()
            }),
            mto_oper_gravadas: Some(amount("100.00")?),
            mto_igv: Some(amount("18.00")?),
            total_impuestos: Some(amount("18.00")?),
            mto_imp_venta: Some(amount("118.00")?),
            forma_pago: Some(PaymentTerms::contado()),
            legends: Some(vec![Legend {
                code: Some("1000".into()),
                value: Some("SON CIENTO DIECIOCHO CON 00/100 SOLES".into()),
            }]),
            details: Some(vec![SaleDetail {
                cod_producto: Some("P001".into()),
                unidad: Some("NIU".into()),
                cantidad: Some(amount("2")?),
                descripcion: Some("PRODUCTO".into()),
                mto_valor_unitario: Some(amount("50.00")?),
                mto_base_igv: Some(amount("100.00")?),
                porcentaje_igv: Some(amount("18")?),
                igv: Some(amount("18.00")?),
                tip_afe_igv: Some("10".into()),
                total_impuestos: Some(amount("18.00")?),
                mto_valor_venta: Some(amount("100.00")?),
                mto_precio_unitario: Some(amount("59.00")?),
                ..SaleDetail::default()
            }]),
            ..BaseSale::default()
        },
        tipo_operacion: Some("0101".into()),
        valor_venta: Some(amount("100.00")?),
        sub_total: Some(amount("118.00")?),
        ..Invoice::default()
    })
}

/// Regla sintética para Invoice: versión UBL, serie según tipo y total.
fn rule(serie_regex: &str) -> String {
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"
  xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2"
  xmlns:php="http://php.net/xsl"><xsl:param name="nombreArchivoEnviado"/>
  <xsl:template match="/*">
    <xsl:if test="not(php:function('{MATCHES_HANDLER}', '^(2.1)$', string(cbc:UBLVersionID)) = 1)">
      <xsl:message>1|2074|version|Invoice/UBLVersionID|<xsl:value-of select="cbc:UBLVersionID"/></xsl:message>
    </xsl:if>
    <xsl:if test="not(php:function('{MATCHES_HANDLER}', '{serie_regex}', string(cbc:ID)) = 1)">
      <xsl:message>1|1001|serie|Invoice/ID|<xsl:value-of select="cbc:ID"/></xsl:message>
    </xsl:if>
    <xsl:message>2|4000|total|<xsl:value-of select="$nombreArchivoEnviado"/>|<xsl:value-of select="cac:LegalMonetaryTotal/cbc:PayableAmount"/></xsl:message>
  </xsl:template>
</xsl:stylesheet>"#
    )
}

#[test]
fn cpeforge_xml_output_validates_and_round_trips_through_xml_parser() -> TestResult {
    let rules =
        std::env::temp_dir().join(format!("cpeforge-cpe-integration-{}", std::process::id()));
    fs::create_dir_all(rules.join("2.X"))?;
    fs::write(
        rules.join(XslPathResolver::relative_path(DocumentType::Factura)),
        rule("^F[A-Z0-9]{3}-[0-9]{1,8}$"),
    )?;
    fs::write(
        rules.join(XslPathResolver::relative_path(DocumentType::Boleta)),
        rule("^B[A-Z0-9]{3}-[0-9]{1,8}$"),
    )?;
    let validator = CpeValidator::new(&rules);
    let parser = InvoiceParser::with_options(ParseOptions::new());

    for (tipo, serie, ubl, expected_type, serie_ok) in [
        ("01", "F001", "2.1", DocumentType::Factura, true),
        ("03", "B001", "2.1", DocumentType::Boleta, true),
        ("03", "F001", "2.1", DocumentType::Boleta, false),
        ("01", "F002", "2.0", DocumentType::Factura, true),
    ] {
        let model = invoice(tipo, serie, ubl)?;
        let xml = InvoiceBuilder::new(RenderOptions::default()).render(&model)?;
        let filename = format!("{}.xml", model.name()?);
        let document = XmlDocument::parse(&xml)?;
        assert_eq!(XmlTypeResolver.resolve(&document)?, Some(expected_type));

        let errors = validator.validate_xml(&filename, &xml)?;
        let id = format!("{serie}-123");
        let mut expected = Vec::new();
        if ubl != "2.1" {
            expected.push((
                "2074",
                ErrorLevel::Exception,
                "Invoice/UBLVersionID".to_owned(),
                ubl.to_owned(),
            ));
        }
        if !serie_ok {
            expected.push((
                "1001",
                ErrorLevel::Exception,
                "Invoice/ID".to_owned(),
                id.clone(),
            ));
        }
        expected.push((
            "4000",
            ErrorLevel::Observation,
            filename.clone(),
            "118.00".to_owned(),
        ));
        let got: Vec<_> = errors
            .iter()
            .map(|e| {
                (
                    e.code.clone().unwrap_or_default(),
                    e.level,
                    e.node_path.clone().unwrap_or_default(),
                    e.node_value.clone().unwrap_or_default(),
                )
            })
            .collect();
        let want: Vec<_> = expected
            .iter()
            .map(|(c, l, p, v)| ((*c).to_owned(), Some(*l), p.clone(), v.clone()))
            .collect();
        assert_eq!(got, want, "{tipo} {serie}");

        // la referencia solo lee UBL 2.0: se comprueba que lee
        // el mismo XML con el tipo y la serie del modelo.
        if ubl != "2.0" {
            continue;
        }
        let parsed = parser.parse(xml.as_bytes())?.ok_or("sin documento")?;
        assert_eq!(parsed.base.tipo_doc.as_deref(), Some(expected_type.code()));
        assert_eq!(parsed.base.serie.as_deref(), Some(serie));
    }
    fs::remove_dir_all(&rules)?;
    Ok(())
}
