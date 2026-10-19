//! Contratos que implementan los crates de XML, envío, parseo, reportes y
//! validación.
//!
//! Son traits sin implementación en este crate. Cada uno equivale a una
//! interfaz de la referencia; las diferencias de forma son propias de Rust:
//!
//! - los fallos se devuelven como `Result` con un tipo de error asociado, en
//!   lugar de excepciones;
//! - los retornos nulables de la referencia (`?string`, `?BaseResult`, …) son `Option`;
//! - los traits que reciben un documento son genéricos en `D`, con
//!   `dyn Document` por defecto, para admitir tanto implementaciones
//!   específicas (por ejemplo, un builder de facturas) como genéricas.

use crate::Document;
use crate::model::response::ServiceResult;

/// Genera el contenido de un documento.
pub trait Builder<D: ?Sized = dyn Document> {
    /// Error de construcción.
    type Error;

    /// Contenido del archivo, normalmente XML (`build()`); `None` si no hay contenido.
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn build(&self, document: &D) -> Result<Option<String>, Self::Error>;
}

/// Construye y envía un documento.
pub trait Factory<D: ?Sized = dyn Document> {
    /// Resultado del envío.
    type Output: ServiceResult;
    /// Error del proceso.
    type Error;

    /// Envía el documento (`send()`).
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn send(&self, document: &D) -> Result<Option<Self::Output>, Self::Error>;
}

/// Convierte una entrada (XML u otra) en un documento
///.
pub trait DocumentParser {
    /// Tipo de entrada; en la referencia el parámetro no tiene tipo (`$value`).
    type Input: ?Sized;
    /// Documento producido.
    type Output: Document;
    /// Error de parseo.
    type Error;

    /// Interpreta la entrada (`parse()`).
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn parse(&self, value: &Self::Input) -> Result<Option<Self::Output>, Self::Error>;
}

/// Representa un documento, por ejemplo en HTML.
pub trait Report<D: ?Sized = dyn Document> {
    /// Parámetros de presentación; en la referencia es un `array` cuyo valor por defecto es `[]`.
    type Parameters;
    /// Error de representación.
    type Error;

    /// Contenido generado (`render()`).
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn render(
        &self,
        document: &D,
        parameters: &Self::Parameters,
    ) -> Result<Option<String>, Self::Error>;
}

/// Valida un documento.
pub trait DocumentValidator<D: ?Sized = dyn Document> {
    /// Resultado de la validación; en la referencia es un `object` (lista de violaciones).
    type Output;
    /// Error del validador.
    type Error;

    /// Valida el documento (`validate()`).
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn validate(&self, document: &D) -> Result<Option<Self::Output>, Self::Error>;
}

/// Catálogo de mensajes por código de error
///.
pub trait ErrorCodeProvider {
    /// Error al consultar el catálogo.
    type Error;

    /// Todos los pares `(código, mensaje)` en el orden del catálogo (`getAll()`).
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn all(&self) -> Result<Option<Vec<(String, String)>>, Self::Error>;

    /// Mensaje de un código (`getValue()`).
    ///
    /// # Errors
    ///
    /// El error propio de la implementación.
    fn value(&self, code: Option<&str>) -> Result<Option<String>, Self::Error>;
}
