//! Adaptador de los modelos de `cpeforge-core` a valores de plantilla.
//!
//! Cada modelo se expone como un [`ModelObject`] con una lista fija y
//! ordenada de getters (`getTipoDoc`, …), incluidos los calculados
//! (`getName`, `getXmlId`). La plantilla los lee por el nombre del getter,
//! por su nombre corto con inicial minúscula (`doc.tipoDoc`) o sin distinguir
//! mayúsculas. Un atributo que el modelo no tiene queda indefinido:
//! `doc.totalAnticipos is defined` es falso en una nota de crédito.
//!
//! Tipos de los valores: importes como `f64` (ver [`crate::format`]), fechas
//! como [`DateObject`], enteros, booleanos, cadenas, nulos y listas.

use std::fmt;
use std::sync::Arc;

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::despatch::{
    AdditionalDoc, Despatch, DespatchDetail, Direction, Driver, Puerto, Shipment, Transportist,
    Vehicle,
};
use cpeforge_core::model::perception::{Perception, PerceptionDetail};
use cpeforge_core::model::retention::{Exchange, Payment, Retention, RetentionDetail};
use cpeforge_core::model::sale::{
    BaseSale, Charge, Cuota, DetailAttribute, Detraction, EmbededDespatch, Invoice, Legend, Note,
    PaymentTerms, Prepayment, Receipt, RelatedDocument, SaleDetail, SalePerception,
};
use cpeforge_core::model::summary::{Summary, SummaryDetail, SummaryPerception};
use cpeforge_core::model::voided::{Reversion, Voided, VoidedDetail};
use cpeforge_core::{CoreError, Decimal, Document, Zoned};
use minijinja::value::{Enumerator, Object, ObjectRepr, Value};
use minijinja::{Error, State};

use crate::error::{RenderCause, engine_error};
use crate::filters::GetterFailure;
use crate::format;

/// Modelo visto desde la plantilla: clase y getters.
#[derive(Debug)]
pub(crate) struct ModelObject {
    class: &'static str,
    attrs: Attrs,
}

/// Getter de un modelo: nombre y cálculo del valor.
type Getter<T> = (&'static str, fn(&T) -> Value);

/// Getters de un [`ModelObject`].
enum Attrs {
    /// Valores ya calculados.
    Eager(Vec<(&'static str, Value)>),
    /// Fila de una colección de detalles: el valor se calcula al leerse. Las
    /// plantillas solo leen unos pocos de los getters de cada línea.
    Row(Box<dyn RowSource>),
}

impl fmt::Debug for Attrs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eager(attrs) => f
                .debug_list()
                .entries(attrs.iter().map(|(n, _)| n))
                .finish(),
            Self::Row(_) => f.write_str("Row"),
        }
    }
}

/// Getters perezosos de una fila.
trait RowSource: Send + Sync {
    /// Nombre del getter `index`, si existe.
    fn method(&self, index: usize) -> Option<&'static str>;
    /// Valor del getter `index`.
    fn value(&self, index: usize) -> Option<Value>;
}

/// Fila `index` de una colección compartida por todas sus filas.
struct Row<T: 'static> {
    items: Arc<[T]>,
    index: usize,
    getters: &'static [Getter<T>],
}

impl<T: Send + Sync> RowSource for Row<T> {
    fn method(&self, index: usize) -> Option<&'static str> {
        self.getters.get(index).map(|(name, _)| *name)
    }

    fn value(&self, index: usize) -> Option<Value> {
        let (_, getter) = self.getters.get(index)?;
        self.items.get(self.index).map(getter)
    }
}

/// Nombre corto de un getter, sin el prefijo `get`, `is` o `has`:
/// `getTipoDoc` → `TipoDoc`.
fn short_name(method: &str) -> Option<&str> {
    ["get", "is", "has"]
        .iter()
        .find_map(|prefix| method.strip_prefix(prefix))
        .filter(|rest| !rest.is_empty())
}

/// `short` con la inicial en minúscula es igual a `item` (`TipoDoc` →
/// `tipoDoc`), sin construir la cadena.
fn lower_first_matches(short: &str, item: &str) -> bool {
    let (short, item) = (short.as_bytes(), item.as_bytes());
    match (short.split_first(), item.split_first()) {
        (Some((first, rest)), Some((item_first, item_rest))) => {
            first.to_ascii_lowercase() == *item_first && rest == item_rest
        }
        _ => false,
    }
}

impl ModelObject {
    /// Nombre corto de la clase (`Invoice`, `Client`, …).
    pub(crate) fn class(&self) -> &'static str {
        self.class
    }

    /// Valor que devuelve el método `method` (`getClient`), si la clase lo tiene.
    pub(crate) fn call(&self, method: &str) -> Option<Value> {
        self.position(|name| name == method)
            .and_then(|index| self.value_at(index))
    }

    fn method_at(&self, index: usize) -> Option<&'static str> {
        match &self.attrs {
            Attrs::Eager(attrs) => attrs.get(index).map(|(name, _)| *name),
            Attrs::Row(row) => row.method(index),
        }
    }

    fn value_at(&self, index: usize) -> Option<Value> {
        match &self.attrs {
            Attrs::Eager(attrs) => attrs.get(index).map(|(_, value)| value.clone()),
            Attrs::Row(row) => row.value(index),
        }
    }

    /// Primer getter, en el orden de la clase, cuyo nombre cumple `matches`.
    fn position(&self, matches: impl Fn(&str) -> bool) -> Option<usize> {
        (0..)
            .map_while(|index| self.method_at(index).map(|name| (index, name)))
            .find(|(_, name)| matches(name))
            .map(|(index, _)| index)
    }

    fn lookup(&self, item: &str) -> Option<Value> {
        // Primero coincidencia exacta (nombre del getter o nombre corto con
        // inicial minúscula), después sin distinguir mayúsculas.
        self.position(|method| {
            method == item || short_name(method).is_some_and(|s| lower_first_matches(s, item))
        })
        .or_else(|| {
            self.position(|method| {
                method.eq_ignore_ascii_case(item)
                    || short_name(method).is_some_and(|s| s.eq_ignore_ascii_case(item))
            })
        })
        .and_then(|index| self.value_at(index))
    }
}

impl Object for ModelObject {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.lookup(key.as_str()?)
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        // Un objeto sin propiedades públicas no produce elementos en `for`.
        Enumerator::Empty
    }

    fn enumerator_len(self: &Arc<Self>) -> Option<usize> {
        None
    }

    fn is_true(self: &Arc<Self>) -> bool {
        true
    }

    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.class)
    }
}

/// Fecha con zona horaria: expone `timestamp`, `offset` y `format(…)`.
#[derive(Debug)]
pub(crate) struct DateObject(pub(crate) Zoned);

impl Object for DateObject {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        let key = key.as_str()?;
        let is = |names: [&str; 2]| names.iter().any(|n| n.eq_ignore_ascii_case(key));
        if is(["getTimestamp", "timestamp"]) {
            Some(Value::from(self.0.timestamp().as_second()))
        } else if is(["getOffset", "offset"]) {
            Some(Value::from(self.0.offset().seconds()))
        } else {
            None
        }
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Empty
    }

    fn is_true(self: &Arc<Self>) -> bool {
        true
    }

    fn call_method(
        self: &Arc<Self>,
        _state: &mut State<'_, '_>,
        method: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        if method.eq_ignore_ascii_case("format") {
            let format = args.first().and_then(Value::as_str).ok_or_else(|| {
                engine_error(RenderCause::TypeError {
                    function: "DateTimeInterface::format",
                })
            })?;
            return Ok(Value::from(format::format_date(&self.0, format)));
        }
        Err(Error::from(minijinja::ErrorKind::UnknownMethod))
    }

    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Constructor de [`ModelObject`].
struct Builder {
    class: &'static str,
    attrs: Vec<(&'static str, Value)>,
}

fn null() -> Value {
    Value::from(())
}

fn text(value: Option<&String>) -> Value {
    value.map_or_else(null, |v| Value::from(v.as_str()))
}

fn amount(value: Option<Decimal>) -> Value {
    value.map_or_else(null, |v| Value::from(format::decimal_to_f64(v)))
}

fn date(value: Option<&Zoned>) -> Value {
    value.map_or_else(null, |v| Value::from_object(DateObject(v.clone())))
}

fn list<T>(value: Option<&Vec<T>>, f: fn(&T) -> Value) -> Value {
    value.map_or_else(null, |items| {
        Value::from(items.iter().map(f).collect::<Vec<_>>())
    })
}

/// Filas a partir de las cuales una colección de detalles se representa con
/// objetos perezosos. Con menos, copiar las filas y recalcular cada getter
/// leído cuesta más que calcularlos todos (medido con `benches/render.rs`).
const LAZY_ROWS_MIN: usize = 17;

/// Colección de detalles con los getters de `getters`. Las colecciones
/// grandes comparten una copia de las filas y calculan cada getter al leerlo;
/// las pequeñas se calculan completas. El resultado observable es el mismo.
fn rows<T: Clone + Send + Sync + 'static>(
    value: Option<&Vec<T>>,
    class: &'static str,
    getters: &'static [Getter<T>],
) -> Value {
    value.map_or_else(null, |items| {
        if items.len() < LAZY_ROWS_MIN {
            return Value::from(
                items
                    .iter()
                    .map(|item| {
                        Value::from_object(ModelObject {
                            class,
                            attrs: Attrs::Eager(
                                getters
                                    .iter()
                                    .map(|(name, get)| (*name, get(item)))
                                    .collect(),
                            ),
                        })
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let shared: Arc<[T]> = Arc::from(items.as_slice());
        Value::from(
            (0..shared.len())
                .map(|index| {
                    Value::from_object(ModelObject {
                        class,
                        attrs: Attrs::Row(Box::new(Row {
                            items: Arc::clone(&shared),
                            index,
                            getters,
                        })),
                    })
                })
                .collect::<Vec<_>>(),
        )
    })
}

fn texts(value: Option<&Vec<String>>) -> Value {
    value.map_or_else(null, |items| {
        Value::from(
            items
                .iter()
                .map(|s| Value::from(s.as_str()))
                .collect::<Vec<_>>(),
        )
    })
}

fn object<T>(value: Option<&T>, f: fn(&T) -> Value) -> Value {
    value.map_or_else(null, f)
}

/// Resultado de un getter calculado que puede fallar.
fn computed(result: Result<String, CoreError>) -> Value {
    match result {
        Ok(value) => Value::from(value),
        Err(err) => Value::from_object(GetterFailure(RenderCause::Document(err))),
    }
}

impl Builder {
    fn new(class: &'static str) -> Self {
        Self {
            class,
            attrs: Vec::with_capacity(32),
        }
    }

    fn getter(mut self, method: &'static str, value: Value) -> Self {
        self.attrs.push((method, value));
        self
    }

    fn build(self) -> Value {
        Value::from_object(ModelObject {
            class: self.class,
            attrs: Attrs::Eager(self.attrs),
        })
    }
}

pub(crate) fn address(a: &Address) -> Value {
    Builder::new("Address")
        .getter("getUbigueo", text(a.ubigueo.as_ref()))
        .getter("getCodigoPais", text(a.codigo_pais.as_ref()))
        .getter("getDepartamento", text(a.departamento.as_ref()))
        .getter("getProvincia", text(a.provincia.as_ref()))
        .getter("getDistrito", text(a.distrito.as_ref()))
        .getter("getUrbanizacion", text(a.urbanizacion.as_ref()))
        .getter("getDireccion", text(a.direccion.as_ref()))
        .getter("getCodLocal", text(a.cod_local.as_ref()))
        .build()
}

pub(crate) fn company(c: &Company) -> Value {
    Builder::new("Company")
        .getter("getRuc", text(c.ruc.as_ref()))
        .getter("getRazonSocial", text(c.razon_social.as_ref()))
        .getter("getNombreComercial", text(c.nombre_comercial.as_ref()))
        .getter("getAddress", object(c.address.as_ref(), address))
        .getter("getEmail", text(c.email.as_ref()))
        .getter("getTelephone", text(c.telephone.as_ref()))
        .build()
}

pub(crate) fn client(c: &Client) -> Value {
    Builder::new("Client")
        .getter("getTipoDoc", text(c.tipo_doc.as_ref()))
        .getter("getNumDoc", text(c.num_doc.as_ref()))
        .getter("getRznSocial", text(c.rzn_social.as_ref()))
        .getter("getAddress", object(c.address.as_ref(), address))
        .getter("getEmail", text(c.email.as_ref()))
        .getter("getTelephone", text(c.telephone.as_ref()))
        .build()
}

fn related(d: &RelatedDocument) -> Value {
    Builder::new("Document")
        .getter("getTipoDoc", text(d.tipo_doc.as_ref()))
        .getter("getNroDoc", text(d.nro_doc.as_ref()))
        .build()
}

fn legend(l: &Legend) -> Value {
    Builder::new("Legend")
        .getter("getCode", text(l.code.as_ref()))
        .getter("getValue", text(l.value.as_ref()))
        .build()
}

fn charge(c: &Charge) -> Value {
    Builder::new("Charge")
        .getter("getCodTipo", text(c.cod_tipo.as_ref()))
        .getter("getFactor", amount(c.factor))
        .getter("getMonto", amount(c.monto))
        .getter("getMontoBase", amount(c.monto_base))
        .build()
}

fn attribute(a: &DetailAttribute) -> Value {
    Builder::new("DetailAttribute")
        .getter("getCode", text(a.code.as_ref()))
        .getter("getName", text(a.name.as_ref()))
        .getter("getValue", text(a.value.as_ref()))
        .getter("getFecInicio", date(a.fec_inicio.as_ref()))
        .getter("getFecFin", date(a.fec_fin.as_ref()))
        .getter("getDuracion", a.duracion.map_or_else(null, Value::from))
        .build()
}

/// Getters de `SaleDetail`, en orden; se evalúan al leerse.
static SALE_DETAIL: &[Getter<SaleDetail>] = &[
    ("getUnidad", |d| text(d.unidad.as_ref())),
    ("getCantidad", |d| amount(d.cantidad)),
    ("getCodProducto", |d| text(d.cod_producto.as_ref())),
    ("getCodProdSunat", |d| text(d.cod_prod_sunat.as_ref())),
    ("getCodProdGS1", |d| text(d.cod_prod_gs1.as_ref())),
    ("getDescripcion", |d| text(d.descripcion.as_ref())),
    ("getMtoValorUnitario", |d| amount(d.mto_valor_unitario)),
    ("getCargos", |d| list(d.cargos.as_ref(), charge)),
    ("getDescuentos", |d| list(d.descuentos.as_ref(), charge)),
    ("getDescuento", |d| amount(d.descuento)),
    ("getMtoBaseIgv", |d| amount(d.mto_base_igv)),
    ("getPorcentajeIgv", |d| amount(d.porcentaje_igv)),
    ("getIgv", |d| amount(d.igv)),
    ("getTipAfeIgv", |d| text(d.tip_afe_igv.as_ref())),
    ("getMtoBaseIsc", |d| amount(d.mto_base_isc)),
    ("getPorcentajeIsc", |d| amount(d.porcentaje_isc)),
    ("getIsc", |d| amount(d.isc)),
    ("getTipSisIsc", |d| text(d.tip_sis_isc.as_ref())),
    ("getMtoBaseOth", |d| amount(d.mto_base_oth)),
    ("getPorcentajeOth", |d| amount(d.porcentaje_oth)),
    ("getOtroTributo", |d| amount(d.otro_tributo)),
    ("getIcbper", |d| amount(d.icbper)),
    ("getFactorIcbper", |d| amount(d.factor_icbper)),
    ("getTotalImpuestos", |d| amount(d.total_impuestos)),
    ("getMtoPrecioUnitario", |d| amount(d.mto_precio_unitario)),
    ("getMtoValorVenta", |d| amount(d.mto_valor_venta)),
    ("getMtoValorGratuito", |d| amount(d.mto_valor_gratuito)),
    ("getAtributos", |d| list(d.atributos.as_ref(), attribute)),
];

fn cuota(c: &Cuota) -> Value {
    Builder::new("Cuota")
        .getter("getMoneda", text(c.moneda.as_ref()))
        .getter("getMonto", amount(c.monto))
        .getter("getFechaPago", date(c.fecha_pago.as_ref()))
        .build()
}

fn payment_terms(p: &PaymentTerms) -> Value {
    Builder::new("PaymentTerms")
        .getter("getMoneda", text(p.moneda.as_ref()))
        .getter("getTipo", text(p.tipo.as_ref()))
        .getter("getMonto", amount(p.monto))
        .build()
}

fn sale_perception(p: &SalePerception) -> Value {
    Builder::new("SalePerception")
        .getter("getCodReg", text(p.cod_reg.as_ref()))
        .getter("getPorcentaje", amount(p.porcentaje))
        .getter("getMtoBase", amount(p.mto_base))
        .getter("getMto", amount(p.mto))
        .getter("getMtoTotal", amount(p.mto_total))
        .build()
}

fn prepayment(p: &Prepayment) -> Value {
    Builder::new("Prepayment")
        .getter("getTipoDocRel", text(p.tipo_doc_rel.as_ref()))
        .getter("getNroDocRel", text(p.nro_doc_rel.as_ref()))
        .getter("getTotal", amount(p.total))
        .build()
}

fn detraction(d: &Detraction) -> Value {
    Builder::new("Detraction")
        .getter("getPercent", amount(d.percent))
        .getter("getMount", amount(d.mount))
        .getter("getCtaBanco", text(d.cta_banco.as_ref()))
        .getter("getCodMedioPago", text(d.cod_medio_pago.as_ref()))
        .getter("getCodBienDetraccion", text(d.cod_bien_detraccion.as_ref()))
        .getter("getValueRef", amount(d.value_ref))
        .build()
}

fn direction(d: &Direction) -> Value {
    Builder::new("Direction")
        .getter("getUbigueo", text(d.ubigueo.as_ref()))
        .getter("getDireccion", text(d.direccion.as_ref()))
        .getter("getCodLocal", text(d.cod_local.as_ref()))
        .getter("getRuc", text(d.ruc.as_ref()))
        .build()
}

fn embeded_despatch(e: &EmbededDespatch) -> Value {
    Builder::new("EmbededDespatch")
        .getter("getLlegada", object(e.llegada.as_ref(), direction))
        .getter("getPartida", object(e.partida.as_ref(), direction))
        .getter("getTransportista", object(e.transportista.as_ref(), client))
        .getter("getNroLicencia", text(e.nro_licencia.as_ref()))
        .getter("getTranspPlaca", text(e.transp_placa.as_ref()))
        .getter("getTranspCodeAuth", text(e.transp_code_auth.as_ref()))
        .getter("getTranspMarca", text(e.transp_marca.as_ref()))
        .getter("getModTraslado", text(e.mod_traslado.as_ref()))
        .getter("getPesoBruto", amount(e.peso_bruto))
        .getter("getUndPesoBruto", text(e.und_peso_bruto.as_ref()))
        .build()
}

/// Getters de `BaseSale` (después de los propios de la subclase).
fn base_sale(b: Builder, s: &BaseSale) -> Builder {
    b.getter("getUblVersion", text(s.ubl_version.as_ref()))
        .getter("getTipoDoc", text(s.tipo_doc.as_ref()))
        .getter("getSerie", text(s.serie.as_ref()))
        .getter("getCorrelativo", text(s.correlativo.as_ref()))
        .getter("getFechaEmision", date(s.fecha_emision.as_ref()))
        .getter("getCompany", object(s.company.as_ref(), company))
        .getter("getClient", object(s.client.as_ref(), client))
        .getter("getTipoMoneda", text(s.tipo_moneda.as_ref()))
        .getter("getSumOtrosCargos", amount(s.sum_otros_cargos))
        .getter("getMtoOperGravadas", amount(s.mto_oper_gravadas))
        .getter("getMtoOperInafectas", amount(s.mto_oper_inafectas))
        .getter("getMtoOperExoneradas", amount(s.mto_oper_exoneradas))
        .getter("getMtoOperExportacion", amount(s.mto_oper_exportacion))
        .getter("getMtoOperGratuitas", amount(s.mto_oper_gratuitas))
        .getter("getMtoIGVGratuitas", amount(s.mto_igv_gratuitas))
        .getter("getMtoIGV", amount(s.mto_igv))
        .getter("getMtoBaseIvap", amount(s.mto_base_ivap))
        .getter("getMtoIvap", amount(s.mto_ivap))
        .getter("getMtoBaseIsc", amount(s.mto_base_isc))
        .getter("getMtoISC", amount(s.mto_isc))
        .getter("getMtoBaseOth", amount(s.mto_base_oth))
        .getter("getMtoOtrosTributos", amount(s.mto_otros_tributos))
        .getter("getIcbper", amount(s.icbper))
        .getter("getTotalImpuestos", amount(s.total_impuestos))
        .getter("getRedondeo", amount(s.redondeo))
        .getter("getMtoImpVenta", amount(s.mto_imp_venta))
        .getter(
            "getDetails",
            rows(s.details.as_ref(), "SaleDetail", SALE_DETAIL),
        )
        .getter("getLegends", list(s.legends.as_ref(), legend))
        .getter("getGuias", list(s.guias.as_ref(), related))
        .getter("getRelDocs", list(s.rel_docs.as_ref(), related))
        .getter("getCompra", text(s.compra.as_ref()))
        .getter("getFormaPago", object(s.forma_pago.as_ref(), payment_terms))
        .getter("getCuotas", list(s.cuotas.as_ref(), cuota))
}

pub(crate) fn invoice(i: &Invoice) -> Value {
    let b = Builder::new("Invoice")
        .getter("getTipoOperacion", text(i.tipo_operacion.as_ref()))
        .getter("getFecVencimiento", date(i.fec_vencimiento.as_ref()))
        .getter("getSumDsctoGlobal", amount(i.sum_dscto_global))
        .getter("getMtoDescuentos", amount(i.mto_descuentos))
        .getter("getSumOtrosDescuentos", amount(i.sum_otros_descuentos))
        .getter("getDescuentos", list(i.descuentos.as_ref(), charge))
        .getter("getCargos", list(i.cargos.as_ref(), charge))
        .getter("getMtoCargos", amount(i.mto_cargos))
        .getter("getTotalAnticipos", amount(i.total_anticipos))
        .getter(
            "getPerception",
            object(i.perception.as_ref(), sale_perception),
        )
        .getter(
            "getGuiaEmbebida",
            object(i.guia_embebida.as_ref(), embeded_despatch),
        )
        .getter("getAnticipos", list(i.anticipos.as_ref(), prepayment))
        .getter("getDetraccion", object(i.detraccion.as_ref(), detraction))
        .getter("getSeller", object(i.seller.as_ref(), client))
        .getter("getValorVenta", amount(i.valor_venta))
        .getter("getSubTotal", amount(i.sub_total))
        .getter("getObservacion", text(i.observacion.as_ref()))
        .getter(
            "getDireccionEntrega",
            object(i.direccion_entrega.as_ref(), address),
        );
    base_sale(b, &i.base)
        .getter("getName", computed(i.name()))
        .build()
}

pub(crate) fn note(n: &Note) -> Value {
    let b = Builder::new("Note")
        .getter("getCodMotivo", text(n.cod_motivo.as_ref()))
        .getter("getDesMotivo", text(n.des_motivo.as_ref()))
        .getter("getTipDocAfectado", text(n.tip_doc_afectado.as_ref()))
        .getter("getNumDocfectado", text(n.num_doc_afectado.as_ref()))
        .getter(
            "getPerception",
            object(n.perception.as_ref(), sale_perception),
        )
        .getter("getValorVenta", amount(n.valor_venta))
        .getter("getSubTotal", amount(n.sub_total));
    base_sale(b, &n.base)
        .getter("getName", computed(n.name()))
        .build()
}

pub(crate) fn base_sale_document(s: &BaseSale) -> Value {
    base_sale(Builder::new("BaseSale"), s)
        .getter("getName", computed(s.name()))
        .build()
}

pub(crate) fn receipt(r: &Receipt) -> Value {
    Builder::new("Receipt")
        .getter("getPerson", object(r.person.as_ref(), company))
        .getter("getReceptor", object(r.receptor.as_ref(), client))
        .getter("getSerie", text(r.serie.as_ref()))
        .getter("getCorrelativo", text(r.correlativo.as_ref()))
        .getter("getFechaEmision", date(r.fecha_emision.as_ref()))
        .getter("getConcepto", text(r.concepto.as_ref()))
        .getter("getMontoLetras", text(r.monto_letras.as_ref()))
        .getter("getSubTotal", amount(r.sub_total))
        .getter("getRetencion", amount(r.retencion))
        .getter("getPorcentaje", amount(r.porcentaje))
        .getter("getTotal", amount(r.total))
        .getter("getName", computed(r.name()))
        .build()
}

fn additional_doc(a: &AdditionalDoc) -> Value {
    Builder::new("AdditionalDoc")
        .getter("getTipoDesc", text(a.tipo_desc.as_ref()))
        .getter("getTipo", text(a.tipo.as_ref()))
        .getter("getNro", text(a.nro.as_ref()))
        .getter("getEmisor", text(a.emisor.as_ref()))
        .build()
}

/// Getters de `DespatchDetail`, en orden; se evalúan al leerse.
static DESPATCH_DETAIL: &[Getter<DespatchDetail>] = &[
    ("getCodigo", |d| text(d.codigo.as_ref())),
    ("getDescripcion", |d| text(d.descripcion.as_ref())),
    ("getUnidad", |d| text(d.unidad.as_ref())),
    ("getCantidad", |d| amount(d.cantidad)),
    ("getCodProdSunat", |d| text(d.cod_prod_sunat.as_ref())),
    ("getAtributos", |d| list(d.atributos.as_ref(), attribute)),
];

fn driver(d: &Driver) -> Value {
    Builder::new("Driver")
        .getter("getTipo", text(d.tipo.as_ref()))
        .getter("getTipoDoc", text(d.tipo_doc.as_ref()))
        .getter("getNroDoc", text(d.nro_doc.as_ref()))
        .getter("getNombres", text(d.nombres.as_ref()))
        .getter("getApellidos", text(d.apellidos.as_ref()))
        .getter("getLicencia", text(d.licencia.as_ref()))
        .build()
}

fn puerto(p: &Puerto) -> Value {
    Builder::new("Puerto")
        .getter("getCodigo", text(p.codigo.as_ref()))
        .getter("getNombre", text(p.nombre.as_ref()))
        .build()
}

fn transportist(t: &Transportist) -> Value {
    Builder::new("Transportist")
        .getter("getTipoDoc", text(t.tipo_doc.as_ref()))
        .getter("getNumDoc", text(t.num_doc.as_ref()))
        .getter("getRznSocial", text(t.rzn_social.as_ref()))
        .getter("getNroMtc", text(t.nro_mtc.as_ref()))
        .getter("getPlaca", text(t.placa.as_ref()))
        .getter("getChoferTipoDoc", text(t.chofer_tipo_doc.as_ref()))
        .getter("getChoferDoc", text(t.chofer_doc.as_ref()))
        .build()
}

fn vehicle(v: &Vehicle) -> Value {
    Builder::new("Vehicle")
        .getter("getPlaca", text(v.placa.as_ref()))
        .getter("getNroCirculacion", text(v.nro_circulacion.as_ref()))
        .getter("getCodEmisor", text(v.cod_emisor.as_ref()))
        .getter("getNroAutorizacion", text(v.nro_autorizacion.as_ref()))
        .getter("getSecundarios", list(v.secundarios.as_ref(), vehicle))
        .build()
}

fn shipment(s: &Shipment) -> Value {
    Builder::new("Shipment")
        .getter("getCodTraslado", text(s.cod_traslado.as_ref()))
        .getter("getDesTraslado", text(s.des_traslado.as_ref()))
        .getter("getSustentoPeso", text(s.sustento_peso.as_ref()))
        .getter(
            "isIndTransbordo",
            s.ind_transbordo.map_or_else(null, Value::from),
        )
        .getter("getIndicadores", texts(s.indicadores.as_ref()))
        .getter("getPesoItems", amount(s.peso_items))
        .getter("getPesoTotal", amount(s.peso_total))
        .getter("getUndPesoTotal", text(s.und_peso_total.as_ref()))
        .getter("getNumBultos", s.num_bultos.map_or_else(null, Value::from))
        .getter("getModTraslado", text(s.mod_traslado.as_ref()))
        .getter("getFecTraslado", date(s.fec_traslado.as_ref()))
        .getter("getFecEntregaBienes", date(s.fec_entrega_bienes.as_ref()))
        .getter("getNumContenedor", text(s.num_contenedor.as_ref()))
        .getter("getContenedores", texts(s.contenedores.as_ref()))
        .getter("getCodPuerto", text(s.cod_puerto.as_ref()))
        .getter("getPuerto", object(s.puerto.as_ref(), puerto))
        .getter("getAeropuerto", object(s.aeropuerto.as_ref(), puerto))
        .getter(
            "getTransportista",
            object(s.transportista.as_ref(), transportist),
        )
        .getter("getVehiculo", object(s.vehiculo.as_ref(), vehicle))
        .getter("getChoferes", list(s.choferes.as_ref(), driver))
        .getter("getLlegada", object(s.llegada.as_ref(), direction))
        .getter("getPartida", object(s.partida.as_ref(), direction))
        .build()
}

pub(crate) fn despatch(d: &Despatch) -> Value {
    Builder::new("Despatch")
        .getter("getVersion", text(d.version.as_ref()))
        .getter("getTipoDoc", text(d.tipo_doc.as_ref()))
        .getter("getSerie", text(d.serie.as_ref()))
        .getter("getCorrelativo", text(d.correlativo.as_ref()))
        .getter("getObservacion", text(d.observacion.as_ref()))
        .getter("getFechaEmision", date(d.fecha_emision.as_ref()))
        .getter("getCompany", object(d.company.as_ref(), company))
        .getter("getDestinatario", object(d.destinatario.as_ref(), client))
        .getter("getTercero", object(d.tercero.as_ref(), client))
        .getter("getComprador", object(d.comprador.as_ref(), client))
        .getter("getEnvio", object(d.envio.as_ref(), shipment))
        .getter("getDocBaja", object(d.doc_baja.as_ref(), related))
        .getter("getRelDoc", object(d.rel_doc.as_ref(), related))
        .getter("getAddDocs", list(d.add_docs.as_ref(), additional_doc))
        .getter(
            "getDetails",
            rows(d.details.as_ref(), "DespatchDetail", DESPATCH_DETAIL),
        )
        .getter("getName", computed(d.name()))
        .build()
}

fn exchange(e: &Exchange) -> Value {
    Builder::new("Exchange")
        .getter("getMonedaRef", text(e.moneda_ref.as_ref()))
        .getter("getMonedaObj", text(e.moneda_obj.as_ref()))
        .getter("getFactor", amount(e.factor))
        .getter("getFecha", date(e.fecha.as_ref()))
        .build()
}

fn payment(p: &Payment) -> Value {
    Builder::new("Payment")
        .getter("getMoneda", text(p.moneda.as_ref()))
        .getter("getImporte", amount(p.importe))
        .getter("getFecha", date(p.fecha.as_ref()))
        .build()
}

/// Getters de `PerceptionDetail`, en orden; se evalúan al leerse.
static PERCEPTION_DETAIL: &[Getter<PerceptionDetail>] = &[
    ("getTipoDoc", |d| text(d.tipo_doc.as_ref())),
    ("getNumDoc", |d| text(d.num_doc.as_ref())),
    ("getFechaEmision", |d| date(d.fecha_emision.as_ref())),
    ("getImpTotal", |d| amount(d.imp_total)),
    ("getMoneda", |d| text(d.moneda.as_ref())),
    ("getCobros", |d| list(d.cobros.as_ref(), payment)),
    ("getFechaPercepcion", |d| date(d.fecha_percepcion.as_ref())),
    ("getImpPercibido", |d| amount(d.imp_percibido)),
    ("getImpCobrar", |d| amount(d.imp_cobrar)),
    ("getTipoCambio", |d| {
        object(d.tipo_cambio.as_ref(), exchange)
    }),
];

pub(crate) fn perception(p: &Perception) -> Value {
    Builder::new("Perception")
        .getter("getSerie", text(p.serie.as_ref()))
        .getter("getCorrelativo", text(p.correlativo.as_ref()))
        .getter("getFechaEmision", date(p.fecha_emision.as_ref()))
        .getter("getCompany", object(p.company.as_ref(), company))
        .getter("getProveedor", object(p.proveedor.as_ref(), client))
        .getter("getRegimen", text(p.regimen.as_ref()))
        .getter("getTasa", amount(p.tasa))
        .getter("getImpPercibido", amount(p.imp_percibido))
        .getter("getImpCobrado", amount(p.imp_cobrado))
        .getter("getObservacion", text(p.observacion.as_ref()))
        .getter(
            "getDetails",
            rows(p.details.as_ref(), "PerceptionDetail", PERCEPTION_DETAIL),
        )
        .getter("getName", computed(p.name()))
        .build()
}

/// Getters de `RetentionDetail`, en orden; se evalúan al leerse.
static RETENTION_DETAIL: &[Getter<RetentionDetail>] = &[
    ("getTipoDoc", |d| text(d.tipo_doc.as_ref())),
    ("getNumDoc", |d| text(d.num_doc.as_ref())),
    ("getFechaEmision", |d| date(d.fecha_emision.as_ref())),
    ("getImpTotal", |d| amount(d.imp_total)),
    ("getMoneda", |d| text(d.moneda.as_ref())),
    ("getPagos", |d| list(d.pagos.as_ref(), payment)),
    ("getFechaRetencion", |d| date(d.fecha_retencion.as_ref())),
    ("getImpRetenido", |d| amount(d.imp_retenido)),
    ("getImpPagar", |d| amount(d.imp_pagar)),
    ("getTipoCambio", |d| {
        object(d.tipo_cambio.as_ref(), exchange)
    }),
];

pub(crate) fn retention(r: &Retention) -> Value {
    Builder::new("Retention")
        .getter("getSerie", text(r.serie.as_ref()))
        .getter("getCorrelativo", text(r.correlativo.as_ref()))
        .getter("getFechaEmision", date(r.fecha_emision.as_ref()))
        .getter("getCompany", object(r.company.as_ref(), company))
        .getter("getProveedor", object(r.proveedor.as_ref(), client))
        .getter("getRegimen", text(r.regimen.as_ref()))
        .getter("getTasa", amount(r.tasa))
        .getter("getImpRetenido", amount(r.imp_retenido))
        .getter("getImpPagado", amount(r.imp_pagado))
        .getter("getObservacion", text(r.observacion.as_ref()))
        .getter(
            "getDetails",
            rows(r.details.as_ref(), "RetentionDetail", RETENTION_DETAIL),
        )
        .getter("getName", computed(r.name()))
        .build()
}

fn summary_perception(p: &SummaryPerception) -> Value {
    Builder::new("SummaryPerception")
        .getter("getCodReg", text(p.cod_reg.as_ref()))
        .getter("getTasa", amount(p.tasa))
        .getter("getMtoBase", amount(p.mto_base))
        .getter("getMto", amount(p.mto))
        .getter("getMtoTotal", amount(p.mto_total))
        .build()
}

/// Getters de `SummaryDetail`, en orden; se evalúan al leerse.
static SUMMARY_DETAIL: &[Getter<SummaryDetail>] = &[
    ("getTipoDoc", |d| text(d.tipo_doc.as_ref())),
    ("getSerieNro", |d| text(d.serie_nro.as_ref())),
    ("getClienteTipo", |d| text(d.cliente_tipo.as_ref())),
    ("getClienteNro", |d| text(d.cliente_nro.as_ref())),
    ("getDocReferencia", |d| {
        object(d.doc_referencia.as_ref(), related)
    }),
    ("getPercepcion", |d| {
        object(d.percepcion.as_ref(), summary_perception)
    }),
    ("getEstado", |d| text(d.estado.as_ref())),
    ("getTotal", |d| amount(d.total)),
    ("getPorcentajeIgv", |d| amount(d.porcentaje_igv)),
    ("getMtoOperGravadas", |d| amount(d.mto_oper_gravadas)),
    ("getMtoOperInafectas", |d| amount(d.mto_oper_inafectas)),
    ("getMtoOperExoneradas", |d| amount(d.mto_oper_exoneradas)),
    ("getMtoOperExportacion", |d| amount(d.mto_oper_exportacion)),
    ("getMtoOperGratuitas", |d| amount(d.mto_oper_gratuitas)),
    ("getMtoOtrosCargos", |d| amount(d.mto_otros_cargos)),
    ("getMtoIGV", |d| amount(d.mto_igv)),
    ("getMtoIvap", |d| amount(d.mto_ivap)),
    ("getMtoISC", |d| amount(d.mto_isc)),
    ("getMtoOtrosTributos", |d| amount(d.mto_otros_tributos)),
    ("getMtoIcbper", |d| amount(d.mto_icbper)),
];

pub(crate) fn summary(s: &Summary) -> Value {
    Builder::new("Summary")
        .getter("getCorrelativo", text(s.correlativo.as_ref()))
        .getter("getFecGeneracion", date(s.fec_generacion.as_ref()))
        .getter("getFecResumen", date(s.fec_resumen.as_ref()))
        .getter("getMoneda", text(s.moneda.as_ref()))
        .getter("getCompany", object(s.company.as_ref(), company))
        .getter(
            "getDetails",
            rows(s.details.as_ref(), "SummaryDetail", SUMMARY_DETAIL),
        )
        .getter("getXmlId", computed(s.xml_id()))
        .getter("getName", computed(s.name()))
        .build()
}

/// Getters de `VoidedDetail`, en orden; se evalúan al leerse.
static VOIDED_DETAIL: &[Getter<VoidedDetail>] = &[
    ("getTipoDoc", |d| text(d.tipo_doc.as_ref())),
    ("getSerie", |d| text(d.serie.as_ref())),
    ("getCorrelativo", |d| text(d.correlativo.as_ref())),
    ("getDesMotivoBaja", |d| text(d.des_motivo_baja.as_ref())),
];

fn voided_fields(class: &'static str, v: &Voided) -> Builder {
    Builder::new(class)
        .getter("getCorrelativo", text(v.correlativo.as_ref()))
        .getter("getFecGeneracion", date(v.fec_generacion.as_ref()))
        .getter("getFecComunicacion", date(v.fec_comunicacion.as_ref()))
        .getter("getCompany", object(v.company.as_ref(), company))
        .getter(
            "getDetails",
            rows(v.details.as_ref(), "VoidedDetail", VOIDED_DETAIL),
        )
}

pub(crate) fn voided(v: &Voided) -> Value {
    voided_fields("Voided", v)
        .getter("getXmlId", computed(v.xml_id()))
        .getter("getName", computed(v.name()))
        .build()
}

pub(crate) fn reversion(r: &Reversion) -> Value {
    voided_fields("Reversion", &r.voided)
        .getter("getXmlId", computed(r.xml_id()))
        .getter("getName", computed(r.name()))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(value: &Value, key: &str) -> Option<Value> {
        value.get_attr(key).ok().filter(|v| !v.is_undefined())
    }

    /// Todas las formas admitidas de nombrar un getter, y otras que no.
    fn keys(method: &str) -> Vec<String> {
        let mut out = vec![
            method.to_owned(),
            method.to_ascii_lowercase(),
            method.to_ascii_uppercase(),
        ];
        if let Some(short) = short_name(method) {
            let mut lc = short.to_owned();
            lc[..1].make_ascii_lowercase();
            out.extend([lc, short.to_owned(), short.to_ascii_uppercase()]);
        }
        out.extend(["nothing".into(), "getNothing".into(), String::new()]);
        out
    }

    /// Las filas perezosas resuelven y calculan igual que las completas.
    fn lazy_rows_match_eager<T: Clone + Default + Send + Sync + 'static>(
        make: impl Fn(usize) -> T,
        class: &'static str,
        getters: &'static [Getter<T>],
    ) {
        let items: Vec<T> = (0..LAZY_ROWS_MIN + 3).map(make).collect();
        let lazy = rows(Some(&items), class, getters);
        let eager = rows(Some(&items[..LAZY_ROWS_MIN - 1].to_vec()), class, getters);
        for index in 0..LAZY_ROWS_MIN - 1 {
            let l = lazy.get_item(&Value::from(index)).unwrap();
            let e = eager.get_item(&Value::from(index)).unwrap();
            let (lo, eo) = (
                l.downcast_object_ref::<ModelObject>().unwrap(),
                e.downcast_object_ref::<ModelObject>().unwrap(),
            );
            assert!(matches!(lo.attrs, Attrs::Row(_)) && matches!(eo.attrs, Attrs::Eager(_)));
            assert_eq!(lo.class(), eo.class());
            for (method, _) in getters {
                assert_eq!(
                    format!("{:?}", lo.call(method)),
                    format!("{:?}", eo.call(method))
                );
                for key in keys(method) {
                    assert_eq!(
                        format!("{:?}", lo.lookup(&key)),
                        format!("{:?}", eo.lookup(&key)),
                        "{class}.{key}"
                    );
                }
            }
        }
    }

    #[test]
    fn lazy_detail_rows_resolve_like_eager_rows() {
        let n = |i: usize| Some(Decimal::new(i64::try_from(i).unwrap() * 125 + 1, 2));
        let t = |i: usize| Some(format!("<x> {i}"));
        let when: Option<Zoned> = Some("2024-03-15T10:00:00-05:00[America/Lima]".parse().unwrap());
        lazy_rows_match_eager(
            |i| SaleDetail {
                unidad: t(i),
                cantidad: n(i),
                descripcion: t(i),
                mto_valor_venta: n(i),
                igv: n(i),
                cargos: Some(vec![Charge {
                    monto: n(i),
                    ..Charge::default()
                }]),
                atributos: Some(vec![DetailAttribute {
                    fec_inicio: when.clone(),
                    ..DetailAttribute::default()
                }]),
                ..SaleDetail::default()
            },
            "SaleDetail",
            SALE_DETAIL,
        );
        lazy_rows_match_eager(
            |i| DespatchDetail {
                codigo: t(i),
                cantidad: n(i),
                ..DespatchDetail::default()
            },
            "DespatchDetail",
            DESPATCH_DETAIL,
        );
        lazy_rows_match_eager(
            |i| PerceptionDetail {
                num_doc: t(i),
                fecha_emision: when.clone(),
                imp_total: n(i),
                ..PerceptionDetail::default()
            },
            "PerceptionDetail",
            PERCEPTION_DETAIL,
        );
        lazy_rows_match_eager(
            |i| RetentionDetail {
                num_doc: t(i),
                imp_pagar: n(i),
                ..RetentionDetail::default()
            },
            "RetentionDetail",
            RETENTION_DETAIL,
        );
        lazy_rows_match_eager(
            |i| SummaryDetail {
                serie_nro: t(i),
                total: n(i),
                ..SummaryDetail::default()
            },
            "SummaryDetail",
            SUMMARY_DETAIL,
        );
        lazy_rows_match_eager(
            |i| VoidedDetail {
                serie: t(i),
                ..VoidedDetail::default()
            },
            "VoidedDetail",
            VOIDED_DETAIL,
        );
    }

    #[test]
    fn attribute_resolution() {
        let inv = Invoice {
            base: BaseSale {
                tipo_doc: Some("01".into()),
                mto_igv: Some(Decimal::new(1800, 2)),
                ..BaseSale::default()
            },
            ..Invoice::default()
        };
        let doc = invoice(&inv);
        for key in ["tipoDoc", "getTipoDoc", "TIPODOC", "tipodoc", "gettipodoc"] {
            assert_eq!(lookup(&doc, key).unwrap().as_str(), Some("01"), "{key}");
        }
        assert!(lookup(&doc, "mtoIGV").is_some());
        assert!(lookup(&doc, "nothing").is_none());
        // Una nota no tiene totalAnticipos: indefinido.
        let n = note(&Note::default());
        assert!(lookup(&n, "totalAnticipos").is_none());
        assert!(lookup(&n, "numDocfectado").is_some());
        // isIndTransbordo se lee como indTransbordo.
        let s = shipment(&Shipment {
            ind_transbordo: Some(true),
            ..Shipment::default()
        });
        assert!(lookup(&s, "indTransbordo").unwrap().is_true());
        assert!(lookup(&s, "getIndTransbordo").is_none());
    }
}
