//! Emisor y direcciones.

/// Dirección de un emisor o receptor.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Address {
    /// Código de ubigeo INEI (propiedad `ubigueo`).
    pub ubigueo: Option<String>,
    /// Código de país; por defecto `PE`.
    pub codigo_pais: Option<String>,
    /// Departamento.
    pub departamento: Option<String>,
    /// Provincia.
    pub provincia: Option<String>,
    /// Distrito.
    pub distrito: Option<String>,
    /// Urbanización.
    pub urbanizacion: Option<String>,
    /// Dirección detallada.
    pub direccion: Option<String>,
    /// Código de establecimiento anexo SUNAT; por defecto `0000`.
    pub cod_local: Option<String>,
}

impl Default for Address {
    fn default() -> Self {
        Self {
            ubigueo: None,
            codigo_pais: Some("PE".to_owned()),
            departamento: None,
            provincia: None,
            distrito: None,
            urbanizacion: None,
            direccion: None,
            cod_local: Some("0000".to_owned()),
        }
    }
}

/// Emisor del comprobante.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(default, rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Company {
    /// RUC del emisor.
    pub ruc: Option<String>,
    /// Razón social.
    pub razon_social: Option<String>,
    /// Nombre comercial.
    pub nombre_comercial: Option<String>,
    /// Domicilio fiscal.
    pub address: Option<Address>,
    /// Correo electrónico.
    pub email: Option<String>,
    /// Teléfono.
    pub telephone: Option<String>,
}
