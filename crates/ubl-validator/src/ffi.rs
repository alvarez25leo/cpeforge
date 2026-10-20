//! Único módulo con `unsafe`: puente C de libxml2, con buffers y callback acotados.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::{InfrastructureError, XmlError};

static XML_LOCK: Mutex<()> = Mutex::new(());
/// Versión de la libxml2 en tiempo de ejecución, si es la mínima admitida.
static XML_INIT: OnceLock<Result<(), String>> = OnceLock::new();

/// libxml2 mínima: 2.9.11 corrige CVE de XSD y del analizador (CVE-2020-24977,
/// CVE-2021-3517/3518/3537) y añade el límite de operaciones XPath.
pub(crate) const MIN_LIBXML_VERSION: u32 = 20911;

/// Serializa las llamadas a libxml2 e inicializa la biblioteca la primera
/// vez, comprobando la versión cargada.
fn lock() -> Result<MutexGuard<'static, ()>, InfrastructureError> {
    let init = XML_INIT.get_or_init(|| {
        // SAFETY: `cpeforge_init` solo llama a `xmlInitParser`; `OnceLock` y
        // el candado global impiden dos llamadas concurrentes (también con
        // cpeforge-cpe-validator), que antes de libxml2 2.11 no son seguras.
        // `cpeforge_libxml_version` devuelve una cadena estática.
        let version = cpeforge_xml_limits::with_libxml_init_lock(|| unsafe {
            cpeforge_init();
            CStr::from_ptr(cpeforge_libxml_version())
                .to_string_lossy()
                .into_owned()
        });
        let number: u32 = version
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        if number >= MIN_LIBXML_VERSION {
            Ok(())
        } else {
            Err(format!(
                "libxml2 {MIN_LIBXML_VERSION} o posterior requerida; cargada: {version}"
            ))
        }
    });
    if let Err(message) = init {
        return Err(InfrastructureError::SchemaValidate(vec![XmlError {
            level: 3,
            code: 0,
            column: 0,
            message: message.clone(),
            file: None,
            line: 0,
        }]));
    }
    Ok(XML_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner))
}

unsafe extern "C" {
    fn cpeforge_init();
    fn cpeforge_libxml_version() -> *const c_char;
    fn cpeforge_preflight_xml(
        xml: *const u8,
        length: c_int,
        max_nodes: c_int,
        max_depth: c_int,
    ) -> c_int;
    fn cpeforge_probe_xml(
        xml: *const u8,
        length: c_int,
        name: *mut c_char,
        name_cap: c_int,
        version: *mut c_char,
        version_cap: c_int,
    ) -> c_int;
    fn cpeforge_schema_validate(
        xml: *const u8,
        length: c_int,
        schema_path: *const c_char,
        callback: unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, c_int, *const c_char),
        user: *mut c_void,
    ) -> c_int;
}

pub(crate) fn preflight(
    xml: &[u8],
    max_nodes: usize,
    max_depth: usize,
) -> Result<(), InfrastructureError> {
    let _guard = lock()?;
    let length = c_int::try_from(xml.len()).map_err(|_| InfrastructureError::XmlLimit)?;
    let max_nodes = c_int::try_from(max_nodes).map_err(|_| InfrastructureError::XmlLimit)?;
    let max_depth = c_int::try_from(max_depth).map_err(|_| InfrastructureError::XmlLimit)?;
    // SAFETY: el slice permanece vivo durante la llamada y C libera el DOM.
    let result = unsafe { cpeforge_preflight_xml(xml.as_ptr(), length, max_nodes, max_depth) };
    match result {
        1 => Ok(()),
        0 => Err(InfrastructureError::XmlParse),
        -1 => Err(InfrastructureError::DtdForbidden),
        _ => Err(InfrastructureError::XmlLimit),
    }
}

pub(crate) fn probe(xml: &[u8]) -> Result<Option<(String, String)>, InfrastructureError> {
    let _guard = lock()?;
    let length = c_int::try_from(xml.len()).map_err(|_| InfrastructureError::XmlLimit)?;
    let mut name = [0 as c_char; 1024];
    let mut version = [0 as c_char; 1024];
    // SAFETY: los dos buffers son mutables, con capacidad declarada exacta;
    // C siempre termina con NUL antes de devolver 1 y libera el DOM.
    let result = unsafe {
        cpeforge_probe_xml(
            xml.as_ptr(),
            length,
            name.as_mut_ptr(),
            1024,
            version.as_mut_ptr(),
            1024,
        )
    };
    match result {
        0 => Ok(None),
        -2 => Err(InfrastructureError::DtdForbidden),
        1 => {
            // SAFETY: el puente C escribió terminadores NUL dentro de ambos buffers.
            let name = unsafe { CStr::from_ptr(name.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: mismo contrato del buffer `version`.
            let version = unsafe { CStr::from_ptr(version.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            Ok(Some((name, version)))
        }
        _ => Err(InfrastructureError::XmlLimit),
    }
}

unsafe extern "C" fn collect(
    user: *mut c_void,
    level: c_int,
    code: c_int,
    column: c_int,
    line: c_int,
    message: *const c_char,
) {
    if user.is_null() || message.is_null() {
        return;
    }
    // SAFETY: `user` points to the live Vec passed synchronously below; C
    // never retains it and libxml invokes the callback before returning.
    let errors = unsafe { &mut *(user.cast::<Vec<XmlError>>()) };
    if errors.len() >= 256 {
        return;
    }
    // SAFETY: libxml owns a NUL-terminated message for this callback's lifetime.
    let message = unsafe { CStr::from_ptr(message) }
        .to_string_lossy()
        .into_owned();
    errors.push(XmlError {
        level,
        code,
        column,
        line,
        message,
        file: None,
    });
}

pub(crate) fn validate(
    xml: &[u8],
    path: &str,
) -> Result<(bool, Vec<XmlError>), InfrastructureError> {
    let _guard = lock()?;
    let path = CString::new(path).map_err(|_| InfrastructureError::UnsafePath)?;
    let length = c_int::try_from(xml.len()).map_err(|_| InfrastructureError::XmlLimit)?;
    let mut errors = Vec::<XmlError>::new();
    // SAFETY: xml is live for the entire call, path is NUL-terminated, the
    // callback only writes to `errors` synchronously, and C releases every
    // parser/schema/document context before returning.
    let result = unsafe {
        cpeforge_schema_validate(
            xml.as_ptr(),
            length,
            path.as_ptr(),
            collect,
            (&mut errors as *mut Vec<XmlError>).cast::<c_void>(),
        )
    };
    match result {
        1 => Ok((true, errors)),
        0 => Ok((false, errors)),
        -1 => Err(InfrastructureError::XmlParse),
        -2 => Err(InfrastructureError::SchemaCompile(errors)),
        -4 => Err(InfrastructureError::DtdForbidden),
        _ => Err(InfrastructureError::SchemaValidate(errors)),
    }
}
