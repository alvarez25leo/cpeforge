//! Datos compartidos por los casos.
//!
//! - [`Contexto`]: la fecha base y los RUC de las credenciales. Las fechas
//!   relativas del demo (`new DateTime('-3days')`) se calculan desde la fecha
//!   base, para que la referencia y Rust firmen el mismo XML.
//! - [`empresa`] y [`cliente`]: copia de `CPEForge\Data\SharedStore` de
//!   la referencia, que usa el demo (sus otros datos no los usa ningún caso).

use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::{Address, Company};
use cpeforge_core::model::sale::{BaseSale, SaleDetail};
use cpeforge_core::{Decimal, Zoned};
use jiff::ToSpan;

/// Fecha base y RUC de las credenciales.
pub struct Contexto {
    fecha: Zoned,
    /// RUC de la Clave SOL (emisor de los casos de inicio).
    pub ruc: String,
    /// RUC del emisor de guías en la API GRE de prueba.
    pub gre_ruc: String,
}

impl Contexto {
    /// Contexto con la fecha base (hora de Lima).
    pub fn new(fecha: Zoned, ruc: String, gre_ruc: String) -> Self {
        Self {
            fecha,
            ruc,
            gre_ruc,
        }
    }

    /// Fecha base.
    pub fn fecha(&self) -> Option<Zoned> {
        Some(self.fecha.clone())
    }

    /// `new DateTime('+N days')` del demo, desde la fecha base.
    ///
    /// # Errors
    ///
    /// Fecha fuera del rango de `jiff`.
    pub fn dias(&self, dias: i64) -> Result<Option<Zoned>, jiff::Error> {
        Ok(Some(self.fecha.checked_add(dias.days())?))
    }

    /// Medianoche del día `dias` después de la fecha base.
    ///
    /// # Errors
    ///
    /// Fecha fuera del rango de `jiff`.
    pub fn medianoche(&self, dias: i64) -> Result<Option<Zoned>, jiff::Error> {
        let dia = self.fecha.checked_add(dias.days())?;
        Ok(Some(dia.date().to_zoned(dia.time_zone().clone())?))
    }
}

/// Texto opcional, como los setters `?string` de la referencia.
pub fn s(value: &str) -> Option<String> {
    Some(value.to_owned())
}

/// Importe exacto: `d(17236, 2)` es 172.36 (`setX(172.36)` en la referencia).
pub fn d(mantissa: i64, scale: u32) -> Option<Decimal> {
    Some(Decimal::new(mantissa, scale))
}

/// `CPEForge\Data\SharedStore::getCompany()`.
pub fn empresa() -> Option<Company> {
    Some(Company {
        ruc: s("20123456789"),
        nombre_comercial: s("CPEFORGE"),
        razon_social: s("CPEFORGE S.A.C."),
        address: Some(Address {
            ubigueo: s("150101"),
            distrito: s("LIMA"),
            provincia: s("LIMA"),
            departamento: s("LIMA"),
            urbanizacion: s("CASUARINAS"),
            cod_local: s("0000"),
            direccion: s("AV NEW DEÁL 123"),
            ..Address::default()
        }),
        email: s("admin@cpeforge.dev"),
        telephone: s("01-234455"),
    })
}

fn direccion(texto: &str) -> Option<Address> {
    Some(Address {
        direccion: s(texto),
        ..Address::default()
    })
}

/// `SharedStore::getClient()`.
pub fn cliente() -> Option<Client> {
    Some(Client {
        tipo_doc: s("6"),
        num_doc: s("20000000001"),
        rzn_social: s("EMPRESA 1 S.A.C."),
        address: direccion(
            "JR. NIQUEL MZA. F LOTE. 3 URB.  INDUSTRIAL INFAÑTAS - LIMA - LIMA -PERU",
        ),
        email: s("client@corp.com"),
        telephone: s("01-445566"),
    })
}

/// Emisor de las guías (`Util::getGRECompany()` del demo).
pub fn empresa_gre(c: &Contexto) -> Option<Company> {
    Some(Company {
        ruc: Some(c.gre_ruc.clone()),
        razon_social: s("CPEFORGE S.A.C."),
        ..Company::default()
    })
}

// Casos de inicio (factura gravada y exonerada),
// emitidos con el RUC de las credenciales.

/// Emisor de los casos de inicio.
pub fn inicio_emisor(ruc: &str) -> Option<Company> {
    Some(Company {
        ruc: s(ruc),
        razon_social: s("GREEN SAC"),
        nombre_comercial: s("GREEN"),
        address: Some(Address {
            ubigueo: s("150101"),
            departamento: s("LIMA"),
            provincia: s("LIMA"),
            distrito: s("LIMA"),
            urbanizacion: s("-"),
            direccion: s("Av. Villa Nueva 221"),
            // Código de establecimiento SUNAT, 0000 por defecto.
            cod_local: s("0000"),
            ..Address::default()
        }),
        email: s("ventas@green.example"),
        telephone: s("01-2345678"),
    })
}

/// Cliente con RUC de los casos de inicio.
pub fn inicio_cliente_empresa() -> Option<Client> {
    Some(Client {
        tipo_doc: s("6"), // RUC - Catálogo 06
        num_doc: s("20123456789"),
        rzn_social: s("EMPRESA X S.A.C."),
        address: direccion("Jr. Niquel 453, San Miguel - Lima"),
        email: s("compras@empresax.example"),
        telephone: s("01-4455667"),
    })
}

/// Cliente con DNI de los casos de inicio.
pub fn inicio_cliente_persona() -> Option<Client> {
    Some(Client {
        tipo_doc: s("1"), // DNI - Catálogo 06
        num_doc: s("48285071"),
        rzn_social: s("NIPAO GUVI"),
        address: direccion("Calle Fusión 453, San Miguel - Lima"),
        ..Client::default()
    })
}

/// Línea gravada (catálogo 07: 10) con IGV 18 %.
pub fn inicio_linea_gravada(
    codigo: &str,
    descripcion: &str,
    cantidad: Decimal,
    valor_unitario: Decimal,
    valor_venta: Decimal,
    igv: Decimal,
    precio_unitario: Decimal,
) -> SaleDetail {
    SaleDetail {
        cod_producto: s(codigo),
        unidad: s("NIU"), // Unidad - Catálogo 03
        cantidad: Some(cantidad),
        mto_valor_unitario: Some(valor_unitario),
        descripcion: s(descripcion),
        mto_base_igv: Some(valor_venta),
        porcentaje_igv: d(18, 0),
        igv: Some(igv),
        tip_afe_igv: s("10"), // Gravado - Operación onerosa
        total_impuestos: Some(igv),
        mto_valor_venta: Some(valor_venta),
        mto_precio_unitario: Some(precio_unitario),
        ..SaleDetail::default()
    }
}

/// Bolsa plástica gravada con ICBPER (S/ 0.50 por bolsa desde 2023).
pub fn inicio_linea_bolsa(
    cantidad: Decimal,
    valor_venta: Decimal,
    igv: Decimal,
    icbper: Decimal,
) -> SaleDetail {
    SaleDetail {
        cod_producto: s("BOLSA"),
        unidad: s("NIU"),
        cantidad: Some(cantidad),
        descripcion: s("BOLSA DE PLASTICO"),
        mto_valor_unitario: d(50, 2),
        mto_precio_unitario: d(59, 2),
        mto_valor_venta: Some(valor_venta),
        tip_afe_igv: s("10"),
        mto_base_igv: Some(valor_venta),
        porcentaje_igv: d(18, 0),
        igv: Some(igv),
        icbper: Some(icbper), // cantidad × factor ICBPER
        factor_icbper: d(50, 2),
        total_impuestos: Some(igv + icbper),
        ..SaleDetail::default()
    }
}

/// Cabecera común de los casos de inicio.
pub fn inicio_base(
    tipo_doc: &str,
    serie: &str,
    correlativo: &str,
    c: &Contexto,
    client: Option<Client>,
) -> BaseSale {
    BaseSale {
        ubl_version: s("2.1"),
        tipo_doc: s(tipo_doc), // 01 factura, 03 boleta - Catálogo 01
        serie: s(serie),
        correlativo: s(correlativo),
        fecha_emision: c.fecha(),
        tipo_moneda: s("PEN"), // Sol - Catálogo 02
        company: inicio_emisor(&c.ruc),
        client,
        ..BaseSale::default()
    }
}
