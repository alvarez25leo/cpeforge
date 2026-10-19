//! Pila: `roxmltree` analiza los elementos de forma recursiva y sin límite, y
//! un desbordamiento de pila aborta el proceso (no es un pánico capturable).
//! Estas pruebas lo reproducen en un proceso hijo con un hilo de 2 MiB (la
//! pila de los workers de Tokio) y comprueban que `XmlDocument` rechaza el
//! mismo documento antes de que el parser recurra, en debug y en release.

use std::process::Command;
use std::thread;

use cpeforge_xml_parser::{Limits, XmlDocument, XmlParserError};

/// Pila de los workers de Tokio y del hilo principal en Linux.
const SMALL_STACK: usize = 2 * 1024 * 1024;
/// Muy por encima del desbordamiento medido de `roxmltree` con 2 MiB
/// (~120 niveles en debug, ~3 100 en release en Apple M5).
const HOSTILE_DEPTH: usize = 20_000;
/// Variable que activa el modo hijo de [`probe_child`].
const PROBE: &str = "CPEFORGE_XML_PARSER_STACK_PROBE";

fn nested(depth: usize) -> String {
    format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth))
}

fn in_small_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    match thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(work)
        .map(thread::JoinHandle::join)
    {
        Ok(Ok(value)) => value,
        Ok(Err(panic)) => std::panic::resume_unwind(panic),
        Err(err) => unreachable!("no se pudo crear el hilo: {err}"),
    }
}

/// Modo hijo: analiza el documento hostil directamente con `roxmltree`, sin el
/// pre-escaneo. En el proceso de pruebas normal no hace nada.
#[test]
fn probe_child() {
    if std::env::var_os(PROBE).is_none() {
        return;
    }
    let xml = nested(HOSTILE_DEPTH);
    let parsed = in_small_stack(move || roxmltree::Document::parse(&xml).is_ok());
    // Solo se llega aquí si `roxmltree` dejó de recurrir: el padre lo detecta.
    println!("roxmltree-sobrevivio={parsed}");
}

#[test]
fn roxmltree_aborts_without_prescan() {
    let exe = std::env::current_exe().unwrap_or_else(|err| unreachable!("{err}"));
    let output = Command::new(exe)
        .args(["probe_child", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE, "1")
        .output()
        .unwrap_or_else(|err| unreachable!("no se pudo lanzar el hijo: {err}"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Si este assert falla, `roxmltree` ya no desborda la pila con este
    // documento: revisar si el pre-escaneo y sus límites siguen haciendo falta.
    assert!(
        !output.status.success(),
        "el hijo terminó bien: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        assert!(
            output.status.signal().is_some(),
            "se esperaba una señal (SIGABRT/SIGSEGV), no un fallo de prueba: {stderr}"
        );
    }
    assert!(stderr.contains("overflowed its stack"), "{stderr}");
}

#[test]
fn hostile_depth_is_rejected_in_small_stack() {
    let err = in_small_stack(|| XmlDocument::parse(nested(HOSTILE_DEPTH).as_bytes()).err());
    let limit = Limits::default().max_depth;
    assert_eq!(err, Some(XmlParserError::TooDeep { limit }));
}

#[test]
fn default_depth_fits_in_small_stack() {
    // El límite por defecto debe caber en 2 MiB también sin optimizar.
    let limit = Limits::default().max_depth;
    let ok = in_small_stack(move || XmlDocument::parse(nested(limit).as_bytes()).is_ok());
    assert!(ok);
}
