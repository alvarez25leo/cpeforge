//! `CPEForge\Validator\Xml`: `XslValidator` y `CpeValidator`.

use std::ffi::CString;
use std::path::{Path, PathBuf};

use crate::entity::CpeError;
use crate::error::{CpeValidatorError, InputSource, LimitKind};
use crate::ffi::{self, TransformSpec};
use crate::options::Limits;
use crate::parser::{ErrorResultParser, ResultParser};
use crate::resolver::{RuleResolver, TypeResolver, XmlTypeResolver, XslPathResolver};
use crate::stylesheet::{Stylesheet, denial_error};
use crate::xml::XmlDocument;

/// Valida un documento (`ValidatorInterface`).
pub trait Validator {
    /// Errores de regla en el orden en que la hoja los emite; vacío si el
    /// documento cumple las reglas.
    ///
    /// # Errors
    ///
    /// Cuando no se pudo validar. Nunca se devuelve `Ok(vec![])` por un
    /// fallo de infraestructura.
    fn validate(
        &self,
        filename: &str,
        document: &XmlDocument,
    ) -> Result<Vec<CpeError>, CpeValidatorError>;
}

/// Ejecuta una hoja concreta (la parte de `XslValidatorInterface` que usa
/// `CpeValidator` tras `setXsl`).
pub trait StylesheetValidator {
    /// Aplica `stylesheet` a `document` con `nombreArchivoEnviado = filename`.
    ///
    /// # Errors
    ///
    /// Igual que [`Validator::validate`].
    fn validate_with(
        &self,
        stylesheet: &Stylesheet,
        filename: &str,
        document: &XmlDocument,
    ) -> Result<Vec<CpeError>, CpeValidatorError>;
}

/// `XslValidator`: ejecuta la hoja, recoge los `xsl:message` y demás
/// diagnósticos de libxslt en orden y los convierte con el
/// [`ResultParser`]. Los mensajes sin formato de error se descartan; el
/// árbol de salida de la transformación no se usa.
///
/// Igual que en la referencia, la hoja configurada con [`XslValidator::set_xsl`] se
/// conserva entre llamadas. A diferencia de la referencia, un fallo no deja estado
/// global (en la referencia una excepción dentro de la transformación deja errores de
/// libxml pendientes que aparecen en la siguiente validación).
#[derive(Debug)]
pub struct XslValidator<P = ErrorResultParser> {
    parser: P,
    stylesheet: Option<Stylesheet>,
    limits: Limits,
}

impl Default for XslValidator {
    fn default() -> Self {
        Self::new(ErrorResultParser)
    }
}

impl<P: ResultParser> XslValidator<P> {
    /// Validador sin hoja.
    #[must_use]
    pub fn new(parser: P) -> Self {
        Self {
            parser,
            stylesheet: None,
            limits: Limits::default(),
        }
    }

    /// Cambia los límites de ejecución.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// `setXsl`: hoja para las siguientes validaciones.
    pub fn set_xsl(&mut self, stylesheet: Stylesheet) {
        self.stylesheet = Some(stylesheet);
    }

    /// Hoja configurada.
    #[must_use]
    pub fn stylesheet(&self) -> Option<&Stylesheet> {
        self.stylesheet.as_ref()
    }

    /// Límites de ejecución.
    #[must_use]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
}

impl<P: ResultParser> Validator for XslValidator<P> {
    fn validate(
        &self,
        filename: &str,
        document: &XmlDocument,
    ) -> Result<Vec<CpeError>, CpeValidatorError> {
        let stylesheet = self
            .stylesheet
            .as_ref()
            .ok_or(CpeValidatorError::XslNotSet)?;
        self.validate_with(stylesheet, filename, document)
    }
}

impl<P: ResultParser> StylesheetValidator for XslValidator<P> {
    fn validate_with(
        &self,
        stylesheet: &Stylesheet,
        filename: &str,
        document: &XmlDocument,
    ) -> Result<Vec<CpeError>, CpeValidatorError> {
        let filename = CString::new(filename).map_err(|_| CpeValidatorError::InvalidFilename)?;
        let spec = TransformSpec {
            filename: &filename,
            op_limit: self.limits.xpath_operations,
            match_limit: self.limits.pcre_backtrack_limit,
            depth_limit: self.limits.pcre_recursion_limit,
        };
        let outcome = ffi::transform(
            stylesheet.style(),
            &document.doc,
            &spec,
            &stylesheet.read_policy(),
        )
        .map_err(CpeValidatorError::Engine)?;
        let session = outcome.session;
        let errors: Vec<CpeError> = session
            .messages
            .iter()
            .filter_map(|m| self.parser.parse(m))
            .collect();
        if let Some(call) = session.extension {
            return Err(CpeValidatorError::Extension {
                class: call.class,
                message: call.message,
            });
        }
        if let Some(denial) = session.denials.first() {
            return Err(denial_error(denial));
        }
        if let Some(uri) = session.rejected_dtd.first() {
            return Err(CpeValidatorError::DoctypeForbidden {
                source: InputSource::Included(uri.clone()),
            });
        }
        if session.op_limit_hit {
            return Err(CpeValidatorError::ResourceLimit {
                kind: LimitKind::XPathOperations,
            });
        }
        if session.regex_limits > 0 {
            return Err(CpeValidatorError::ResourceLimit {
                kind: LimitKind::Regex,
            });
        }
        if let Some(failure) = session.regex_errors.into_iter().next() {
            return Err(CpeValidatorError::Regex {
                pattern: failure.pattern,
                message: failure.message,
            });
        }
        if session.template_depth_hit {
            return Err(CpeValidatorError::ResourceLimit {
                kind: LimitKind::TemplateDepth,
            });
        }
        // 1 = XSLT_STATE_ERROR. `xsl:message terminate="yes"` deja STOPPED (2).
        if outcome.state == 1 || session.runtime_error {
            return Err(CpeValidatorError::Transform {
                partial: errors,
                diagnostics: session.messages,
            });
        }
        Ok(errors)
    }
}

/// `CpeValidator`: tipo → ruta de regla → hoja → [`StylesheetValidator`].
///
/// Si no hay tipo o ruta devuelve un único error `0305` (nivel `1`, resto
/// `None`), como la referencia. La hoja se carga en cada validación (la referencia la lee con
/// `DOMDocument::load` en cada llamada), desde el directorio autorizado del
/// [`RuleResolver`].
#[derive(Debug)]
pub struct CpeValidator<T = XmlTypeResolver, R = XslPathResolver, V = XslValidator> {
    type_resolver: T,
    rule_resolver: R,
    xsl_validator: V,
    limits: Limits,
}

impl CpeValidator {
    /// Composición de `CpeValidatorFactory` sobre un directorio local de
    /// reglas con la estructura `1.X/…` y `2.X/…` de `XslPathResolver`.
    #[must_use]
    pub fn new(rules_dir: impl Into<PathBuf>) -> Self {
        Self::with_parts(
            XmlTypeResolver,
            XslPathResolver::new(rules_dir),
            XslValidator::default(),
        )
    }
}

impl<T: TypeResolver, R: RuleResolver, V: StylesheetValidator> CpeValidator<T, R, V> {
    /// Composición con resolutores y validador propios.
    #[must_use]
    pub fn with_parts(type_resolver: T, rule_resolver: R, xsl_validator: V) -> Self {
        Self {
            type_resolver,
            rule_resolver,
            xsl_validator,
            limits: Limits::default(),
        }
    }

    /// Límites para el XML y la carga de hojas (los de ejecución son los
    /// del validador de hojas).
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// `validateFromXml`: analiza `xml` y valida.
    ///
    /// # Errors
    ///
    /// Los de [`XmlDocument::parse`] y [`Validator::validate`].
    pub fn validate_xml(
        &self,
        filename: &str,
        xml: impl AsRef<[u8]>,
    ) -> Result<Vec<CpeError>, CpeValidatorError> {
        let document = XmlDocument::parse_with_limits(xml, &self.limits)?;
        self.validate(filename, &document)
    }

    /// Ruta de la regla que se usaría para `document` (sin cargarla).
    ///
    /// # Errors
    ///
    /// Los de [`TypeResolver::resolve`].
    pub fn rule_path(&self, document: &XmlDocument) -> Result<Option<PathBuf>, CpeValidatorError> {
        Ok(self
            .type_resolver
            .resolve(document)?
            .and_then(|t| self.rule_resolver.rule_path(t)))
    }

    fn rules_root(&self) -> Option<&Path> {
        self.rule_resolver.rules_root()
    }
}

impl<T: TypeResolver, R: RuleResolver, V: StylesheetValidator> Validator for CpeValidator<T, R, V> {
    fn validate(
        &self,
        filename: &str,
        document: &XmlDocument,
    ) -> Result<Vec<CpeError>, CpeValidatorError> {
        let Some(document_type) = self.type_resolver.resolve(document)? else {
            return Ok(vec![CpeError::unknown_rule()]);
        };
        let Some(path) = self.rule_resolver.rule_path(document_type) else {
            return Ok(vec![CpeError::unknown_rule()]);
        };
        let stylesheet = Stylesheet::from_file(&path, self.rules_root(), &self.limits)?;
        self.xsl_validator
            .validate_with(&stylesheet, filename, document)
    }
}
