//! Opciones de los parsers: zona horaria, reloj y límites.

use cpeforge_core::time::peru_time_zone;
use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};

use crate::convert::DateContext;
use crate::dom::Limits;

/// Opciones comunes a los parsers.
///
/// Una fecha sin zona se interpreta en la zona horaria configurada y un texto
/// de fecha vacío toma la hora actual. Ambas cosas son explícitas: por
/// defecto `America/Lima` y el reloj del sistema.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseOptions {
    time_zone: TimeZone,
    now: Option<Timestamp>,
    limits: Limits,
}

impl Default for ParseOptions {
    fn default() -> Self {
        // La base tz va incluida (`tzdb-bundle-always`); el respaldo fijo
        // -05:00 coincide con Lima desde 1994 y solo cambia el nombre.
        let time_zone = peru_time_zone().unwrap_or_else(|_| TimeZone::fixed(Offset::constant(-5)));
        Self {
            time_zone,
            now: None,
            limits: Limits::default(),
        }
    }
}

impl ParseOptions {
    /// Opciones por defecto: `America/Lima`, reloj del sistema y [`Limits::default`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Zona para las fechas sin desplazamiento (`date_default_timezone_set`).
    #[must_use]
    pub fn with_time_zone(mut self, time_zone: TimeZone) -> Self {
        self.time_zone = time_zone;
        self
    }

    /// Instante fijo para las fechas que toman la hora actual (texto de fecha
    /// vacío, retención/percepción sin información). Útil en pruebas.
    #[must_use]
    pub fn with_fixed_now(mut self, now: Timestamp) -> Self {
        self.now = Some(now);
        self
    }

    /// Límites de lectura del XML.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Zona horaria configurada.
    #[must_use]
    pub fn time_zone(&self) -> &TimeZone {
        &self.time_zone
    }

    /// Límites configurados.
    #[must_use]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Contexto de fechas para un parseo: el reloj se lee una vez por
    /// llamada, de modo que todas las fechas «ahora» de un documento coinciden.
    pub(crate) fn date_context(&self) -> DateContext {
        DateContext {
            time_zone: self.time_zone.clone(),
            now: self.now.unwrap_or_else(Timestamp::now),
        }
    }
}
