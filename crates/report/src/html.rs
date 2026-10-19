//! `HtmlReport`: render HTML con las plantillas integradas o del usuario.

use std::path::PathBuf;
use std::sync::Arc;

use cpeforge_core::model::despatch::Despatch;
use cpeforge_core::model::perception::Perception;
use cpeforge_core::model::retention::Retention;
use cpeforge_core::model::sale::{BaseSale, Invoice, Note, Receipt};
use cpeforge_core::model::summary::Summary;
use cpeforge_core::model::voided::{Reversion, Voided};
use cpeforge_core::ports::Report;
use jiff::Timestamp;
use minijinja::syntax::SyntaxConfig;
use minijinja::{AutoEscape, Environment, UndefinedBehavior, Value};

use crate::context;
use crate::error::{LoaderFailure, RenderCause, ReportError, from_engine};
use crate::filters::{self, OrderedMap, RUNTIME_KEY, Runtime, RuntimeValue};
use crate::params::{ParamValue, ReportParameters};
use crate::templates;

/// Plantilla inicial de [`HtmlReport`].
pub const DEFAULT_TEMPLATE: &str = "invoice.html.twig";

/// Opciones de [`HtmlReport`]: carpeta de plantillas, comportamiento del
/// motor y límites de recursos.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReportOptions {
    /// Carpeta de plantillas del usuario, consultada antes que las integradas.
    pub template_dir: Option<PathBuf>,
    /// `strict_variables`: una variable o atributo inexistente es un error.
    pub strict_variables: bool,
    /// `autoescape`: `true` escapa como HTML todo valor impreso que no esté
    /// marcado como seguro; `false` lo imprime tal cual.
    pub autoescape: bool,
    /// Instante que el filtro `date` usa con una fecha nula. `None` usa el
    /// reloj del sistema.
    pub now: Option<Timestamp>,
    /// Tamaño máximo de una plantilla de la carpeta del usuario.
    pub max_template_bytes: u64,
    /// Tamaño máximo de `params.system.logo`; `None` sin límite.
    pub max_logo_bytes: Option<usize>,
    /// Instrucciones máximas del motor por render; `None` sin límite.
    pub fuel: Option<u64>,
}

impl Default for ReportOptions {
    /// Plantillas integradas, sin `strict_variables`, escape HTML, reloj del
    /// sistema; plantillas de hasta 1 MiB y logo de hasta 10 MiB.
    fn default() -> Self {
        Self {
            template_dir: None,
            strict_variables: false,
            autoescape: true,
            now: None,
            max_template_bytes: 1024 * 1024,
            max_logo_bytes: Some(10 * 1024 * 1024),
            fuel: None,
        }
    }
}

impl ReportOptions {
    /// Opciones con una carpeta de plantillas del usuario.
    #[must_use]
    pub fn with_template_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            template_dir: Some(dir.into()),
            ..Self::default()
        }
    }
}

mod sealed {
    use minijinja::Value;

    /// Conversión interna a valor de plantilla.
    pub trait Sealed {
        /// Objeto `doc` de la plantilla.
        fn context_value(&self) -> Value;
        /// Identificador de clase del modelo (`CPEForge\Model\Sale\Invoice`),
        /// usado por el resolver y en sus errores.
        fn class_name(&self) -> &'static str;
    }
}

/// Documento que [`HtmlReport`] sabe representar: los comprobantes de
/// `cpeforge-core`.
pub trait ReportDocument: sealed::Sealed {}

macro_rules! reportable {
    ($($ty:ty => $f:path, $class:literal;)*) => {$(
        impl sealed::Sealed for $ty {
            fn context_value(&self) -> Value {
                $f(self)
            }
            fn class_name(&self) -> &'static str {
                $class
            }
        }
        impl ReportDocument for $ty {}
    )*};
}

reportable! {
    Invoice => context::invoice, "CPEForge\\Model\\Sale\\Invoice";
    Note => context::note, "CPEForge\\Model\\Sale\\Note";
    BaseSale => context::base_sale_document, "CPEForge\\Model\\Sale\\BaseSale";
    Receipt => context::receipt, "CPEForge\\Model\\Sale\\Receipt";
    Despatch => context::despatch, "CPEForge\\Model\\Despatch\\Despatch";
    Perception => context::perception, "CPEForge\\Model\\Perception\\Perception";
    Retention => context::retention, "CPEForge\\Model\\Retention\\Retention";
    Summary => context::summary, "CPEForge\\Model\\Summary\\Summary";
    Voided => context::voided, "CPEForge\\Model\\Voided\\Voided";
    Reversion => context::reversion, "CPEForge\\Model\\Voided\\Reversion";
}

pub(crate) fn class_name(document: &dyn ReportDocument) -> &'static str {
    document.class_name()
}

/// Motor de representación HTML.
///
/// La plantilla inicial es siempre [`DEFAULT_TEMPLATE`]; para otro tipo de
/// documento se elige con [`HtmlReport::set_template`], por ejemplo con
/// [`crate::DefaultTemplateResolver`]. `render` no consulta el resolver.
pub struct HtmlReport {
    env: Environment<'static>,
    template: String,
    now: Option<Timestamp>,
    max_logo_bytes: Option<usize>,
}

impl std::fmt::Debug for HtmlReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HtmlReport")
            .field("template", &self.template)
            .finish_non_exhaustive()
    }
}

impl Default for HtmlReport {
    fn default() -> Self {
        Self::new(ReportOptions::default())
    }
}

impl HtmlReport {
    /// Crea el motor con `options`.
    #[must_use]
    pub fn new(options: ReportOptions) -> Self {
        let mut env = Environment::new();
        // Las plantillas se escriben contando con que se elimine el primer salto
        // de línea tras `%}` y `#}` y se conserve el último del archivo.
        if let Ok(syntax) = SyntaxConfig::builder()
            .trim_blocks(true)
            .keep_trailing_newline(true)
            .build()
        {
            env.set_syntax(syntax);
        }
        let escape = if options.autoescape {
            AutoEscape::Html
        } else {
            AutoEscape::None
        };
        env.set_auto_escape_callback(move |_| escape.clone());
        env.set_undefined_behavior(if options.strict_variables {
            UndefinedBehavior::Strict
        } else {
            UndefinedBehavior::Chainable
        });
        env.set_fuel(options.fuel);
        let dir = options.template_dir.clone().map(Arc::new);
        let max = options.max_template_bytes;
        env.set_loader(move |name| {
            templates::load(dir.as_deref().map(PathBuf::as_path), name, max).map_err(|err| {
                minijinja::Error::new(minijinja::ErrorKind::InvalidOperation, err.to_string())
                    .with_source(LoaderFailure(err))
            })
        });
        filters::register(&mut env);
        Self {
            env,
            template: DEFAULT_TEMPLATE.to_owned(),
            now: options.now,
            max_logo_bytes: options.max_logo_bytes,
        }
    }

    /// Cambia la plantilla que usa [`HtmlReport::render_html`].
    pub fn set_template(&mut self, template: impl Into<String>) {
        self.template = template.into();
    }

    /// Plantilla actual.
    #[must_use]
    pub fn template(&self) -> &str {
        &self.template
    }

    /// Variable global de todas las plantillas, por ejemplo `max_items`.
    pub fn add_global(&mut self, name: impl Into<String>, value: ParamValue) {
        self.env
            .add_global(name.into(), filters::param_value(&value));
    }

    /// HTML del documento.
    ///
    /// # Errors
    ///
    /// [`ReportError`] de carga, sintaxis o render; los fallos de datos llevan
    /// una [`RenderCause`] tipada.
    pub fn render_html(
        &self,
        document: &dyn ReportDocument,
        parameters: &ReportParameters,
    ) -> Result<String, ReportError> {
        if let (Some(limit), Some(logo)) = (self.max_logo_bytes, &parameters.system.logo) {
            if logo.len() > limit {
                return Err(ReportError::Render {
                    template: Some(self.template.clone()),
                    line: None,
                    cause: RenderCause::Limit(format!(
                        "logo de {} bytes (máximo {limit})",
                        logo.len()
                    )),
                    message: "logo demasiado grande".to_owned(),
                });
            }
        }
        let runtime = Runtime {
            now: self.now.unwrap_or_else(Timestamp::now),
        };
        let context = Value::from_object(OrderedMap(vec![
            ("doc".to_owned(), document.context_value()),
            ("params".to_owned(), parameters.to_template_value()),
            (
                RUNTIME_KEY.to_owned(),
                Value::from_object(RuntimeValue(runtime)),
            ),
        ]));
        let template = self
            .env
            .get_template(&self.template)
            .map_err(|e| from_engine(&e))?;
        template.render(context).map_err(|e| from_engine(&e))
    }
}

impl<D: ReportDocument> Report<D> for HtmlReport {
    type Parameters = ReportParameters;
    type Error = ReportError;

    fn render(
        &self,
        document: &D,
        parameters: &ReportParameters,
    ) -> Result<Option<String>, ReportError> {
        self.render_html(document, parameters).map(Some)
    }
}
