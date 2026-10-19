//! Parámetros de presentación de [`crate::HtmlReport::render_html`].
//!
//! Las plantillas integradas leen:
//!
//! | Clave en la plantilla | Campo Rust | Uso |
//! | --- | --- | --- |
//! | `system.logo` | [`SystemParameters::logo`] | bytes de la imagen, `|image_b64` |
//! | `system.hash` | [`SystemParameters::hash`] | resumen (`DigestValue`) |
//! | `system.qr` | [`SystemParameters::qr`] | URL del QR de la guía (`qrUrl`) |
//! | `user.header` | [`UserParameters::header`] | HTML confiable, `|raw` |
//! | `user.footer` | [`UserParameters::footer`] | HTML confiable, `|raw` |
//! | `user.extras` | [`UserParameters::extras`] | filas `name`/`value` |
//! | `user.numIGV` | [`UserParameters::num_igv`] | porcentaje junto al I.G.V. |
//!
//! Un campo `None` es una clave **ausente** en la plantilla (`is defined` es
//! falso). Una clave presente con valor nulo no se puede expresar con estos
//! campos tipados; `other` admite
//! [`ParamValue::Null`] para plantillas propias.

use std::fmt;

use cpeforge_core::Decimal;
use minijinja::Value;

use crate::filters::{OrderedMap, param_value};

/// HTML que el llamador declara confiable.
///
/// Las plantillas imprimen `header` y `footer` con `|raw`, sin escapar. Este
/// tipo obliga a decidirlo de forma explícita: [`TrustedHtml::new`] declara que
/// el texto es HTML seguro y [`TrustedHtml::escape`] convierte texto plano.
#[derive(Clone, PartialEq, Eq)]
pub struct TrustedHtml(String);

impl TrustedHtml {
    /// Marca `html` como confiable: se insertará sin escapar.
    ///
    /// No usar con datos de terceros sin sanear: un `<script>` se ejecutará al
    /// abrir el HTML.
    #[must_use]
    pub fn new(html: impl Into<String>) -> Self {
        Self(html.into())
    }

    /// Escapa texto plano (`&`, `<`, `>`, `"` y `'`) para insertarlo como HTML.
    #[must_use]
    pub fn escape(text: &str) -> Self {
        let mut out = String::with_capacity(text.len());
        crate::filters::escape_html_into(&mut out, text);
        Self(out)
    }

    /// El HTML.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for TrustedHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TrustedHtml").field(&self.0).finish()
    }
}

/// Valor libre de un parámetro o global de plantilla.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ParamValue {
    /// `null`.
    Null,
    /// `bool`.
    Bool(bool),
    /// `int`.
    Int(i64),
    /// Número decimal: la plantilla lo recibe como el `f64` más cercano (ver
    /// [`crate::format::decimal_to_f64`]).
    Decimal(Decimal),
    /// Cadena (se escapa al imprimir, salvo con `|raw`).
    Text(String),
    /// Cadena binaria, como un logo.
    Bytes(Vec<u8>),
    /// Lista.
    List(Vec<ParamValue>),
    /// Array asociativo; conserva el orden de inserción.
    Map(Vec<(String, ParamValue)>),
}

impl From<&str> for ParamValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for ParamValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i64> for ParamValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<bool> for ParamValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<Decimal> for ParamValue {
    fn from(value: Decimal) -> Self {
        Self::Decimal(value)
    }
}

/// Fila de `user.extras`: se imprime `name|upper` y `value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extra {
    /// Etiqueta.
    pub name: String,
    /// Valor.
    pub value: String,
}

impl Extra {
    /// Nueva fila.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// `params.system`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemParameters {
    /// Imagen del logo (PNG, GIF o JPEG). Todas las plantillas la imprimen
    /// con `|image_b64`; sin logo el render falla con
    /// [`crate::RenderCause::TypeError`].
    pub logo: Option<Vec<u8>>,
    /// Resumen de la firma (ver [`crate::xml_utils`]).
    pub hash: Option<String>,
    /// URL del QR de la guía de remisión.
    pub qr: Option<String>,
    /// Otras claves, para plantillas propias.
    pub other: Vec<(String, ParamValue)>,
}

/// `params.user`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserParameters {
    /// Cabecera HTML (`|raw`).
    pub header: Option<TrustedHtml>,
    /// Pie HTML (`|raw`).
    pub footer: Option<TrustedHtml>,
    /// Filas de información adicional.
    pub extras: Option<Vec<Extra>>,
    /// Porcentaje de IGV que se muestra junto al importe (`numIGV`).
    pub num_igv: Option<ParamValue>,
    /// Otras claves, para plantillas propias.
    pub other: Vec<(String, ParamValue)>,
}

/// Parámetros completos: `params.system` y `params.user`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReportParameters {
    /// Datos del sistema emisor.
    pub system: SystemParameters,
    /// Datos de presentación.
    pub user: UserParameters,
}

impl ReportParameters {
    /// Valor `params` de la plantilla: `['system' => […], 'user' => […]]`.
    ///
    /// Se construye directamente desde los campos, sin un árbol intermedio:
    /// el logo se copia una sola vez y las cadenas cortas no reservan memoria.
    pub(crate) fn to_template_value(&self) -> Value {
        let text = |s: &str| Value::from(s);
        let mut system = Vec::with_capacity(3 + self.system.other.len());
        if let Some(logo) = &self.system.logo {
            system.push(("logo".to_owned(), Value::from_bytes(logo.clone())));
        }
        if let Some(hash) = &self.system.hash {
            system.push(("hash".to_owned(), text(hash)));
        }
        if let Some(qr) = &self.system.qr {
            system.push(("qr".to_owned(), text(qr)));
        }
        system.extend(
            self.system
                .other
                .iter()
                .map(|(k, v)| (k.clone(), param_value(v))),
        );

        let mut user = Vec::with_capacity(4 + self.user.other.len());
        if let Some(num) = &self.user.num_igv {
            user.push(("numIGV".to_owned(), param_value(num)));
        }
        if let Some(header) = &self.user.header {
            user.push(("header".to_owned(), text(header.as_str())));
        }
        if let Some(extras) = &self.user.extras {
            let rows: Vec<Value> = extras
                .iter()
                .map(|e| {
                    Value::from_object(OrderedMap(vec![
                        ("name".to_owned(), text(&e.name)),
                        ("value".to_owned(), text(&e.value)),
                    ]))
                })
                .collect();
            user.push(("extras".to_owned(), Value::from(rows)));
        }
        if let Some(footer) = &self.user.footer {
            user.push(("footer".to_owned(), text(footer.as_str())));
        }
        user.extend(
            self.user
                .other
                .iter()
                .map(|(k, v)| (k.clone(), param_value(v))),
        );

        Value::from_object(OrderedMap(vec![
            ("system".to_owned(), Value::from_object(OrderedMap(system))),
            ("user".to_owned(), Value::from_object(OrderedMap(user))),
        ]))
    }
}
