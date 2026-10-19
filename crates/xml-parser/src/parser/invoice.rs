//! Factura y boleta en UBL 2.0 (`InvoiceParser`).
//!
//! A diferencia de los demás parsers, las consultas registran el prefijo
//! `xt` con el espacio de nombres del raíz y distinguen una lista vacía de
//! una consulta sin resultado (prefijo fuera de ámbito). En ese segundo caso
//! las operaciones sobre la lista fallan con el `Error`/`TypeError` del
//! contrato de compatibilidad.

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{
    BaseSale, Detraction, Invoice, Legend, Prepayment, RelatedDocument, SaleDetail, SalePerception,
};

use crate::convert::{self, DateContext, explode_part, loose_eq, to_float};
use crate::dom::Node;
use crate::error::XmlParserError;
use crate::reader::{Lookup, evaluate};

super::parser!(InvoiceParser, Invoice, "Invoice");

const CLASS: &str = "CPEForge\\Xml\\Parser\\InvoiceParser";

/// Resultado de una consulta: `None` si un prefijo no está en ámbito.
type List<'a> = Option<Vec<Node<'a>>>;

struct Xpt<'a> {
    xml: &'a Lookup<'a>,
    root_ns: &'a str,
}

impl<'a> Xpt<'a> {
    fn query(&self, query: &str, context: Option<Node<'a>>) -> Result<List<'a>, XmlParserError> {
        evaluate(self.xml.document, query, context, &[("xt", self.root_ns)])
    }

    /// Valor del primer nodo o `default`; error si la consulta no tuvo
    /// resultado.
    fn def_value(list: List<'a>, default: &str) -> Result<String, XmlParserError> {
        let list = list.ok_or_else(|| type_error("defValue"))?;
        Ok(list.first().map_or_else(|| default.to_owned(), Node::value))
    }

    /// Valor del primer nodo de la consulta, o cadena vacía.
    fn value(&self, query: &str, context: Option<Node<'a>>) -> Result<String, XmlParserError> {
        Self::def_value(self.query(query, context)?, "")
    }

    /// Importe del primer nodo de la consulta, o de `default`.
    fn float(
        &self,
        query: &str,
        context: Option<Node<'a>>,
        default: &str,
    ) -> Result<rust_decimal::Decimal, XmlParserError> {
        to_float(&Self::def_value(self.query(query, context)?, default)?)
    }

    /// Atributo `name` del primer nodo si es un elemento, o cadena vacía.
    fn attribute(
        &self,
        query: &str,
        context: Option<Node<'a>>,
        name: &str,
    ) -> Result<String, XmlParserError> {
        let list = self
            .query(query, context)?
            .ok_or_else(|| type_error("defNodeAttribute"))?;
        Ok(match list.first() {
            Some(node) if node.is_element() => node.attribute(name),
            _ => String::new(),
        })
    }

    /// Primer nodo de la consulta; error si la consulta no tuvo resultado.
    fn item(
        &self,
        query: &str,
        context: Option<Node<'a>>,
    ) -> Result<Option<Node<'a>>, XmlParserError> {
        let list = self.query(query, context)?.ok_or_else(|| {
            XmlParserError::runtime("Error", "Call to a member function item() on false")
        })?;
        Ok(list.first().copied())
    }

    /// Nodos de la consulta; sin resultado, ninguno.
    fn each(
        &self,
        query: &str,
        context: Option<Node<'a>>,
    ) -> Result<Vec<Node<'a>>, XmlParserError> {
        Ok(self.query(query, context)?.unwrap_or_default())
    }
}

fn type_error(method: &str) -> XmlParserError {
    XmlParserError::runtime(
        "TypeError",
        format!(
            "{CLASS}::{method}(): Argument #1 ($nodeList) must be of type DOMNodeList, false given"
        ),
    )
}

fn parse(xml: &Lookup<'_>, dates: &DateContext) -> Result<Invoice, XmlParserError> {
    let root_ns = xml.root().namespace().ok_or_else(|| {
        XmlParserError::runtime(
            "TypeError",
            "DOMXPath::registerNamespace(): Argument #2 ($namespace) must be of type string, null given",
        )
    })?;
    let x = Xpt { xml, root_ns };
    let id = x.value("/xt:Invoice/cbc:ID", None)?;
    let tipo_doc = x.value("/xt:Invoice/cbc:InvoiceTypeCode", None)?;
    let tipo_moneda = x.value("/xt:Invoice/cbc:DocumentCurrencyCode", None)?;
    let fecha_emision = dates.parse(&x.value("/xt:Invoice/cbc:IssueDate", None)?)?;
    let company = company(&x)?;
    let client = client(&x)?;
    let mut inv = Invoice {
        base: BaseSale {
            serie: explode_part(&id, 0),
            correlativo: explode_part(&id, 1),
            tipo_doc: Some(tipo_doc),
            tipo_moneda: Some(tipo_moneda),
            fecha_emision: Some(fecha_emision),
            company: Some(company),
            client: Some(client),
            ..BaseSale::default()
        },
        ..Invoice::default()
    };

    let extensions = x.item("/xt:Invoice/ext:UBLExtensions", None)?;
    let additional = x.item("//sac:AdditionalInformation", extensions)?;
    load_totals(&x, &mut inv, additional)?;
    load_tributos(&x, &mut inv)?;
    let monetary_total = x.item("/xt:Invoice/cac:LegalMonetaryTotal", None)?;
    inv.tipo_operacion = Some(x.value("sac:SUNATTransaction/cbc:ID", additional)?);
    inv.sum_dscto_global = Some(x.float("cbc:AllowanceTotalAmount", monetary_total, "0")?);
    inv.total_anticipos = Some(x.float("cbc:PrepaidAmount", monetary_total, "0")?);
    inv.anticipos = Some(prepayments(&x)?);
    inv.base.mto_otros_tributos = Some(x.float("cbc:ChargeTotalAmount", monetary_total, "0")?);
    inv.base.mto_imp_venta = Some(x.float("cbc:PayableAmount", monetary_total, "0")?);
    inv.base.details = Some(details(&x)?);
    inv.base.legends = Some(legends(&x, additional)?);

    // loadExtras()
    inv.base.compra = Some(x.value("/xt:Invoice/cac:OrderReference/cbc:ID", None)?);
    let due = x.value("/xt:Invoice/cac:PaymentMeans/cbc:PaymentDueDate", None)?;
    if !convert::is_empty(&due) {
        inv.fec_vencimiento = Some(dates.parse(&due)?);
    }
    let mut guias = Vec::new();
    // Una consulta sin resultado cuenta como lista vacía.
    for guia in x.each("/xt:Invoice/cac:DespatchDocumentReference", None)? {
        let at = Some(guia);
        guias.push(RelatedDocument {
            tipo_doc: Some(x.value("cbc:DocumentTypeCode", at)?),
            nro_doc: Some(x.value("cbc:ID", at)?),
        });
    }
    inv.base.guias = Some(guias);
    Ok(inv)
}

fn address<'a>(x: &Xpt<'a>, node: Option<Node<'a>>) -> Result<Option<Address>, XmlParserError> {
    // Una consulta sin resultado cuenta como lista vacía.
    let found = x
        .query("cac:Party/cac:PostalAddress", node)?
        .unwrap_or_default();
    let Some(address) = found.first().copied() else {
        return Ok(None);
    };
    let at = Some(address);
    Ok(Some(Address {
        direccion: Some(x.value("cbc:StreetName", at)?),
        departamento: Some(x.value("cbc:CityName", at)?),
        provincia: Some(x.value("cbc:CountrySubentity", at)?),
        distrito: Some(x.value("cbc:District", at)?),
        ubigueo: Some(x.value("cbc:ID", at)?),
        ..Address::default()
    }))
}

fn company(x: &Xpt<'_>) -> Result<Company, XmlParserError> {
    let node = x.item("/xt:Invoice/cac:AccountingSupplierParty", None)?;
    Ok(Company {
        ruc: Some(x.value("cbc:CustomerAssignedAccountID", node)?),
        nombre_comercial: Some(x.value("cac:Party/cac:PartyName/cbc:Name", node)?),
        razon_social: Some(x.value("cac:Party/cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        address: address(x, node)?,
        ..Company::default()
    })
}

fn client(x: &Xpt<'_>) -> Result<Client, XmlParserError> {
    let node = x.item("/xt:Invoice/cac:AccountingCustomerParty", None)?;
    Ok(Client {
        num_doc: Some(x.value("cbc:CustomerAssignedAccountID", node)?),
        tipo_doc: Some(x.value("cbc:AdditionalAccountID", node)?),
        rzn_social: Some(x.value("cac:Party/cac:PartyLegalEntity/cbc:RegistrationName", node)?),
        address: address(x, node)?,
        ..Client::default()
    })
}

fn load_totals<'a>(
    x: &Xpt<'a>,
    inv: &mut Invoice,
    node: Option<Node<'a>>,
) -> Result<(), XmlParserError> {
    if node.is_none() {
        return Ok(());
    }
    for total in x.each("sac:AdditionalMonetaryTotal", node)? {
        let at = Some(total);
        let id = x.value("cbc:ID", at)?;
        let id = convert::trim(&id);
        let val = x.float("cbc:PayableAmount", at, "0")?;
        if loose_eq(id, "1001") {
            inv.base.mto_oper_gravadas = Some(val);
        } else if loose_eq(id, "1002") {
            inv.base.mto_oper_inafectas = Some(val);
        } else if loose_eq(id, "1003") {
            inv.base.mto_oper_exoneradas = Some(val);
        } else if loose_eq(id, "1004") {
            inv.base.mto_oper_gratuitas = Some(val);
        } else if loose_eq(id, "2001") {
            inv.perception = Some(SalePerception {
                cod_reg: Some(x.attribute("cbc:ID", at, "schemeID")?),
                mto: Some(val),
                mto_base: Some(x.float("sac:ReferenceAmount", at, "0")?),
                mto_total: Some(x.float("sac:TotalAmount", at, "0")?),
                ..SalePerception::default()
            });
        } else if loose_eq(id, "2003") {
            inv.detraccion = Some(Detraction {
                mount: Some(val),
                percent: Some(x.float("cbc:Percent", at, "0")?),
                value_ref: Some(x.float("sac:ReferenceAmount", at, "0")?),
                ..Detraction::default()
            });
        } else if loose_eq(id, "2005") {
            inv.mto_descuentos = Some(val);
        }
    }
    Ok(())
}

fn load_tributos(x: &Xpt<'_>, inv: &mut Invoice) -> Result<(), XmlParserError> {
    for tax in x.each("/xt:Invoice/cac:TaxTotal", None)? {
        let at = Some(tax);
        let name = x.value("cac:TaxSubtotal/cac:TaxCategory/cac:TaxScheme/cbc:Name", at)?;
        let val = x.float("cbc:TaxAmount", at, "0")?;
        if loose_eq(&name, "IGV") {
            inv.base.mto_igv = Some(val);
        } else if loose_eq(&name, "ISC") {
            inv.base.mto_isc = Some(val);
        } else if loose_eq(&name, "OTROS") {
            inv.base.sum_otros_cargos = Some(val);
        }
    }
    Ok(())
}

fn prepayments(x: &Xpt<'_>) -> Result<Vec<Prepayment>, XmlParserError> {
    let mut out = Vec::new();
    for node in x.each("/xt:Invoice/cac:PrepaidPayment", None)? {
        let at = Some(node);
        out.push(Prepayment {
            total: Some(x.float("cbc:PaidAmount", at, "0")?),
            tipo_doc_rel: Some(x.attribute("cbc:ID", at, "schemeID")?),
            nro_doc_rel: Some(x.value("cbc:ID", at)?),
        });
    }
    Ok(out)
}

fn legends<'a>(x: &Xpt<'a>, node: Option<Node<'a>>) -> Result<Vec<Legend>, XmlParserError> {
    let mut out = Vec::new();
    if node.is_none() {
        return Ok(out);
    }
    for legend in x.each("sac:AdditionalProperty", node)? {
        let at = Some(legend);
        out.push(Legend {
            code: Some(x.value("cbc:ID", at)?),
            value: Some(x.value("cbc:Value", at)?),
        });
    }
    Ok(out)
}

fn details(x: &Xpt<'_>) -> Result<Vec<SaleDetail>, XmlParserError> {
    let mut out = Vec::new();
    for node in x.each("/xt:Invoice/cac:InvoiceLine", None)? {
        let at = Some(node);
        let mut det = SaleDetail {
            cantidad: Some(x.float("cbc:InvoicedQuantity", at, "0")?),
            unidad: Some(x.attribute("cbc:InvoicedQuantity", at, "unitCode")?),
            mto_valor_venta: Some(x.float("cbc:LineExtensionAmount", at, "")?),
            mto_valor_unitario: Some(x.float("cac:Price/cbc:PriceAmount", at, "")?),
            descripcion: Some(x.value("cac:Item/cbc:Description", at)?),
            cod_producto: Some(x.value("cac:Item/cac:SellersItemIdentification/cbc:ID", at)?),
            cod_prod_sunat: Some(x.value(
                "cac:Item/cac:CommodityClassification/cbc:ItemClassificationCode",
                at,
            )?),
            ..SaleDetail::default()
        };
        for tax in x.each("cac:TaxTotal", at)? {
            let tax = Some(tax);
            let name = x.value(
                "cac:TaxSubtotal/cac:TaxCategory/cac:TaxScheme/cbc:Name",
                tax,
            )?;
            let val = x.float("cbc:TaxAmount", tax, "0")?;
            if loose_eq(&name, "IGV") {
                det.igv = Some(val);
                det.tip_afe_igv = Some(x.value(
                    "cac:TaxSubtotal/cac:TaxCategory/cbc:TaxExemptionReasonCode",
                    tax,
                )?);
            } else if loose_eq(&name, "ISC") {
                det.isc = Some(val);
                det.tip_sis_isc =
                    Some(x.value("cac:TaxSubtotal/cac:TaxCategory/cbc:TierRange", tax)?);
            }
        }
        for desc in x.each("cac:AllowanceCharge", at)? {
            let desc = Some(desc);
            let charge = x.value("cbc:ChargeIndicator", desc)?;
            if loose_eq(convert::trim(&charge), "false") {
                det.descuento = Some(x.float("cbc:Amount", desc, "0")?);
            }
        }
        for price in x.each("cac:PricingReference", at)? {
            let price = Some(price);
            let code = x.value("cac:AlternativeConditionPrice/cbc:PriceTypeCode", price)?;
            let value = x.float("cac:AlternativeConditionPrice/cbc:PriceAmount", price, "0")?;
            let code = convert::trim(&code);
            if loose_eq(code, "01") {
                det.mto_precio_unitario = Some(value);
            } else if loose_eq(code, "02") {
                det.mto_valor_gratuito = Some(value);
            }
        }
        out.push(det);
    }
    Ok(out)
}
