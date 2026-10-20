//! Validación de una plantilla al cargarla.
//!
//! MiniJinja resuelve filtros, tests y funciones al ejecutar: una llamada
//! inexistente en una rama que no se ejecuta no falla. Aquí se compila cada
//! plantilla que se carga (la principal y las de `include`, `extends` e
//! `import`) y se recorren sus instrucciones: un filtro, test o función que no
//! existe se rechaza con [`ReportError::Syntax`] antes de renderizar, aunque
//! esté en una rama muerta.
//!
//! Usa la API `minijinja::machinery` (feature `unstable_machinery`); por eso la
//! versión de MiniJinja está fijada con `=` en el workspace.

use std::collections::HashSet;

use minijinja::machinery::{self, Instruction, Instructions};
use minijinja::syntax::SyntaxConfig;
use minijinja::{Environment, ErrorKind};

use crate::error::ReportError;
use crate::filters;

/// Nombres que el motor resuelve sin declararlos: `caller()` dentro de una
/// macro llamada con `{% call %}` y `super()` dentro de un bloque.
const IMPLICIT_FUNCTIONS: [&str; 2] = ["caller", "super"];

/// Comprueba que `source` solo use filtros, tests y funciones existentes.
///
/// Un error de sintaxis no se informa aquí: lo devuelve el motor al compilar
/// la plantilla, con su mensaje habitual.
///
/// # Errors
///
/// [`ReportError::Syntax`] con el primer nombre desconocido.
pub(crate) fn check(name: &str, source: &str, syntax: &SyntaxConfig) -> Result<(), ReportError> {
    let mut env = Environment::new();
    env.set_syntax(syntax.clone());
    filters::register(&mut env);
    let Ok(template) = env.template_from_named_str(name, source) else {
        return Ok(());
    };
    let undeclared = template.undeclared_variables(false);
    let globals: HashSet<&str> = env.globals().map(|(global, _)| global).collect();
    let mut state = env.empty_state();
    let compiled = machinery::get_compiled_template(&template);
    let unknown = |line: Option<usize>, kind: &str, called: &str| ReportError::Syntax {
        template: Some(name.to_owned()),
        line,
        message: format!("Unknown \"{called}\" {kind} in \"{name}\""),
    };
    let mut check_instructions = |instructions: &Instructions<'_>| -> Result<(), ReportError> {
        let mut index = 0;
        while let Some(instruction) = instructions.get(index) {
            let line = instructions.get_line(index);
            match instruction {
                Instruction::ApplyFilter(filter, ..) => {
                    if is_kind(
                        state.apply_filter(filter, &[]).err(),
                        ErrorKind::UnknownFilter,
                    ) {
                        return Err(unknown(line, "filter", filter));
                    }
                }
                Instruction::PerformTest(test, ..) => {
                    if is_kind(state.perform_test(test, &[]).err(), ErrorKind::UnknownTest) {
                        return Err(unknown(line, "test", test));
                    }
                }
                // Una macro o variable declarada en la plantilla no figura
                // entre las no declaradas.
                Instruction::CallFunction(function, _)
                    if undeclared.contains(*function)
                        && !globals.contains(function)
                        && !IMPLICIT_FUNCTIONS.contains(function) =>
                {
                    return Err(unknown(line, "function", function));
                }
                _ => {}
            }
            index += 1;
        }
        Ok(())
    };
    check_instructions(&compiled.instructions)?;
    for block in compiled.blocks.values() {
        check_instructions(block)?;
    }
    Ok(())
}

fn is_kind(error: Option<minijinja::Error>, kind: ErrorKind) -> bool {
    error.is_some_and(|error| error.kind() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Result<(), ReportError> {
        check("t.html.twig", source, &SyntaxConfig::default())
    }

    fn unknown_message(source: &str) -> Option<String> {
        match run(source) {
            Err(ReportError::Syntax { message, .. }) => Some(message),
            _ => None,
        }
    }

    #[test]
    fn unknown_names_fail_even_in_dead_branches() {
        assert_eq!(
            unknown_message("{% if false %}{{ inexistente() }}{% endif %}ok").as_deref(),
            Some("Unknown \"inexistente\" function in \"t.html.twig\"")
        );
        assert_eq!(
            unknown_message("{% if false %}{{ x|nofiltro }}{% endif %}").as_deref(),
            Some("Unknown \"nofiltro\" filter in \"t.html.twig\"")
        );
        assert_eq!(
            unknown_message("{% if x is notest %}{% endif %}").as_deref(),
            Some("Unknown \"notest\" test in \"t.html.twig\"")
        );
        assert!(unknown_message("{% block b %}{{ y|otro }}{% endblock %}").is_some());
    }

    #[test]
    fn known_and_declared_names_pass() {
        for source in [
            "{{ doc.serie|upper|n_format(2) }}{{ qrUrl('x') }}{{ range(3)|length }}",
            "{% if x is defined and x is twig_true %}{{ x|default('-') }}{% endif %}",
            "{% macro m(a) %}{{ a }}{{ caller() }}{% endmacro %}{% call m(1) %}c{% endcall %}{{ m(2) }}",
            "{% set f = namespace(v=1) %}{{ f.v }}{% for i in range(2) %}{{ loop.index }}{% endfor %}",
            "{% if %}sintaxis rota",
        ] {
            assert!(run(source).is_ok(), "{source}");
        }
    }
}
