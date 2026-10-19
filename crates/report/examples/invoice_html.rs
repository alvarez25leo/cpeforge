//! Representación impresa de una factura sintética.
//!
//! `cargo run -p cpeforge-report --example invoice_html > factura.html`

use cpeforge_core::Decimal;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{BaseSale, Invoice, Legend, SaleDetail};
use cpeforge_core::ports::Report;
use cpeforge_report::{
    DefaultTemplateResolver, Extra, HtmlReport, ReportError, ReportParameters, TemplateResolver,
    TrustedHtml, XmlUtils,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let invoice = Invoice {
        base: BaseSale {
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("123".into()),
            fecha_emision: Some("2024-03-15T10:20:30-05:00[America/Lima]".parse()?),
            tipo_moneda: Some("PEN".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                razon_social: Some("EMPRESA SINTETICA S.A.C.".into()),
                address: Some(Address {
                    direccion: Some("AV. FICTICIA 123".into()),
                    ..Address::default()
                }),
                ..Company::default()
            }),
            client: Some(Client {
                tipo_doc: Some("6".into()),
                num_doc: Some("20000000002".into()),
                rzn_social: Some("CLIENTE SINTETICO S.A.".into()),
                ..Client::default()
            }),
            mto_oper_gravadas: Some(Decimal::new(10000, 2)),
            mto_igv: Some(Decimal::new(1800, 2)),
            mto_imp_venta: Some(Decimal::new(11800, 2)),
            details: Some(vec![SaleDetail {
                unidad: Some("NIU".into()),
                cantidad: Some(Decimal::new(2, 0)),
                cod_producto: Some("P001".into()),
                descripcion: Some("PRODUCTO SINTETICO".into()),
                mto_valor_unitario: Some(Decimal::new(5000, 2)),
                mto_valor_venta: Some(Decimal::new(10000, 2)),
                ..SaleDetail::default()
            }]),
            legends: Some(vec![Legend {
                code: Some("1000".into()),
                value: Some("SON CIENTO DIECIOCHO CON 00/100 SOLES".into()),
            }]),
            ..BaseSale::default()
        },
        ..Invoice::default()
    };

    let mut params = ReportParameters::default();
    // Logo PNG sintético (solo la firma; un logo real se lee de un archivo).
    params.system.logo = Some(b"\x89PNG\r\n\x1a\n".to_vec());
    // El resumen se obtiene del XML firmado; aquí uno sintético.
    let signed = r#"<Invoice xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><ds:Signature><ds:SignedInfo><ds:Reference><ds:DigestValue>RESUMEN=</ds:DigestValue></ds:Reference></ds:SignedInfo></ds:Signature></ext:ExtensionContent></ext:UBLExtension></ext:UBLExtensions></Invoice>"#;
    params.system.hash = Some(XmlUtils::default().hash_sign(signed.as_bytes())?);
    params.user.header = Some(TrustedHtml::new("Telf: <b>(01) 000-0000</b>"));
    params.user.extras = Some(vec![Extra::new("Condición de pago", "Contado")]);

    let mut report = HtmlReport::default();
    report.set_template(DefaultTemplateResolver.template(&invoice)?);
    let html: Option<String> = Report::render(&report, &invoice, &params)?;
    println!(
        "{}",
        html.ok_or_else(|| ReportError::TemplateNotFound { name: "?".into() })?
    );
    Ok(())
}
