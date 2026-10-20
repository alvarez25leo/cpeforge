//! `CPEForge\Validator\Entity`: tipo de documento, nivel y error de regla.

use std::fmt;

/// Tipo de comprobante (catálogo 01 y resúmenes), `DocumentType` de la referencia.
///
/// El código no es el nombre de la raíz XML: `Invoice` puede ser factura
/// (`01`) o boleta (`03`), y `VoidedDocuments` comunicación de baja (`RA`)
/// o resumen de reversión (`RR`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DocumentType {
    /// `01`
    Factura,
    /// `03`
    Boleta,
    /// `07`
    NotaCredito,
    /// `08`
    NotaDebito,
    /// `RC`
    ResumenDiario,
    /// `RA`
    ComunicacionBaja,
    /// `09`
    GuiaRemision,
    /// `20`
    Retencion,
    /// `40`
    Percepcion,
    /// `RR`
    ResumenReversion,
}

impl DocumentType {
    /// Los diez tipos, en el orden de las constantes.
    pub const ALL: [Self; 10] = [
        Self::Factura,
        Self::Boleta,
        Self::NotaCredito,
        Self::NotaDebito,
        Self::ResumenDiario,
        Self::ComunicacionBaja,
        Self::GuiaRemision,
        Self::Retencion,
        Self::Percepcion,
        Self::ResumenReversion,
    ];

    /// Código de la referencia (`'01'`, `'RC'`…).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Factura => "01",
            Self::Boleta => "03",
            Self::NotaCredito => "07",
            Self::NotaDebito => "08",
            Self::ResumenDiario => "RC",
            Self::ComunicacionBaja => "RA",
            Self::GuiaRemision => "09",
            Self::Retencion => "20",
            Self::Percepcion => "40",
            Self::ResumenReversion => "RR",
        }
    }

    /// Tipo para un código exacto (sensible a mayúsculas y espacios, como la
    /// clave de array de `XslPathResolver`).
    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.code() == code)
    }
}

impl fmt::Display for DocumentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Nivel de un error de regla (`ErrorLevel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorLevel {
    /// `'1'`: rechazo.
    Exception,
    /// `'2'`: observación.
    Observation,
}

impl ErrorLevel {
    /// Código de la referencia (`'1'` o `'2'`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Exception => "1",
            Self::Observation => "2",
        }
    }
}

/// Error devuelto por una regla (`CpeError`). Los cinco campos son
/// opcionales como en la referencia: `None` (null) es distinto de `Some("")`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct CpeError {
    /// Código de la regla (p. ej. `2074`, o `0305` si no hay regla).
    pub code: Option<String>,
    /// Descripción.
    pub message: Option<String>,
    /// Nivel.
    pub level: Option<ErrorLevel>,
    /// Ruta del nodo evaluado.
    pub node_path: Option<String>,
    /// Valor del nodo evaluado.
    pub node_value: Option<String>,
}

impl CpeError {
    /// Error `0305` (tipo o regla desconocidos) que devuelve `CpeValidator`.
    #[must_use]
    pub fn unknown_rule() -> Self {
        Self {
            code: Some("0305".to_owned()),
            level: Some(ErrorLevel::Exception),
            ..Self::default()
        }
    }

    /// `true` si el nivel es [`ErrorLevel::Exception`].
    #[must_use]
    pub fn is_exception(&self) -> bool {
        self.level == Some(ErrorLevel::Exception)
    }
}
