//! Frontera FFI con libxml2/libxslt (vía `csrc/glue.c`) y PCRE2 (vía
//! `pcre2-sys`). Es el único módulo con `unsafe`; cada bloque documenta su
//! invariante. Las demás capas solo usan los envoltorios seguros de aquí.
//!
//! Estado compartido con C: libxslt entrega mensajes y consulta permisos
//! mediante callbacks globales sin contexto útil, así que cada operación del
//! crate abre una [`Session`] en un `thread_local` del hilo que llama. Los
//! manejadores de libxml2 son por hilo y se instalan en cada sesión; el de
//! libxslt (`xsltGenericError`) y el cargador son globales del proceso, así
//! que `glue.c` instala una sola vez enrutadores que entregan cada mensaje o
//! carga a la sesión del hilo que lo produce. Las compilaciones y
//! transformaciones además se serializan con [`ENGINE`], porque las
//! preferencias por defecto de libxslt también son globales.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_double, c_int, c_ulong, c_void};
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::xslt_ext::{self, ExtensionCall};

#[repr(C)]
struct XmlDoc {
    _opaque: [u8; 0],
}

#[repr(C)]
struct XsltStylesheet {
    _opaque: [u8; 0],
}

/// Argumento de `php:function` ya convertido por glue.c (`gcv_arg`).
#[repr(C)]
pub(crate) struct RawArg {
    kind: c_int,
    str_: *const c_char,
    num: c_double,
    boolean: c_int,
}

/// Valor que glue.c deja en la pila XPath (`gcv_ret`).
#[repr(C)]
pub(crate) struct RawRet {
    action: c_int,
    str_: *const c_char,
    boolean: c_int,
}

const ARG_STRING: c_int = 0;
const ARG_BOOLEAN: c_int = 1;
const ARG_NUMBER: c_int = 2;
const ARG_NODESET: c_int = 3;
const RET_STRING: c_int = 0;
const RET_BOOLEAN: c_int = 1;
const RET_NOTHING: c_int = 2;
const RET_SENTINEL: c_int = 3;

unsafe extern "C" {
    fn gcv_init() -> c_int;
    fn gcv_libxml_version() -> *const c_char;
    fn gcv_libxslt_version() -> *const c_char;
    fn gcv_handlers_push() -> *mut c_void;
    fn gcv_handlers_pop(saved: *mut c_void);
    fn gcv_parse(
        buf: *const c_char,
        len: c_int,
        url: *const c_char,
        has_dtd: *mut c_int,
    ) -> *mut XmlDoc;
    fn gcv_parse_stylesheet_doc(
        buf: *const c_char,
        len: c_int,
        path: *const c_char,
        has_dtd: *mut c_int,
    ) -> *mut XmlDoc;
    fn gcv_free_doc(doc: *mut XmlDoc);
    fn gcv_free(ptr: *mut c_void);
    fn gcv_root_name(doc: *mut XmlDoc) -> *const c_char;
    fn gcv_root_text(doc: *mut XmlDoc, expr: *const c_char, out: *mut *mut c_char) -> c_int;
    fn gcv_compile(doc: *mut XmlDoc, errors: *mut c_int) -> *mut XsltStylesheet;
    fn gcv_free_stylesheet(style: *mut XsltStylesheet);
    fn gcv_transform(
        style: *mut XsltStylesheet,
        src: *mut XmlDoc,
        param_name: *const c_char,
        param_value: *const c_char,
        op_limit: c_ulong,
        state: *mut c_int,
        limit_applied: *mut c_int,
    ) -> c_int;
    fn gcv_op_limit_code() -> c_int;
    fn gcv_op_limit_supported() -> c_int;
}

/// Serializa compilación y transformación (estado global de libxslt).
static ENGINE: Mutex<()> = Mutex::new(());
static INIT: OnceLock<bool> = OnceLock::new();

/// Inicializa libxml2/libexslt y el cargador seguro una sola vez.
fn init() -> Result<(), &'static str> {
    let ok = *INIT.get_or_init(|| {
        cpeforge_xml_limits::with_libxml_init_lock(|| {
            // SAFETY: gcv_init inicializa libxml2 y registra EXSLT, el cargador
            // y las preferencias por defecto; OnceLock garantiza una única
            // ejecución y el candado global, que no coincida con la de
            // cpeforge-ubl-validator.
            unsafe { gcv_init() }
        }) == 0
    });
    if !ok {
        return Err("no se pudo inicializar libxslt");
    }
    let (libxml, libxslt) = runtime_versions();
    if version_number(&libxml) < MIN_LIBXML_VERSION {
        return Err("libxml2 2.9.11 o posterior requerida");
    }
    if version_number(&libxslt) < MIN_LIBXSLT_VERSION {
        return Err("libxslt 1.1.34 o posterior requerida");
    }
    Ok(())
}

/// libxml2 mínima: 2.9.11 corrige CVE del analizador y añade el límite de
/// operaciones XPath (`opLimit`).
const MIN_LIBXML_VERSION: u32 = 20911;
/// libxslt mínima: 1.1.34 corrige CVE-2019-11068 (las preferencias de
/// seguridad se podían saltar).
const MIN_LIBXSLT_VERSION: u32 = 10134;

/// `"20913"` → `20913`; `0` si no empieza por dígitos.
fn version_number(version: &str) -> u32 {
    version
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

fn lock_engine() -> MutexGuard<'static, ()> {
    // Un pánico con el candado tomado no deja estado de Rust inconsistente:
    // la sesión se reinicia en cada operación.
    ENGINE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `true` si la libxml2 enlazada admite `opLimit` (límite de operaciones XPath).
pub(crate) fn op_limit_supported() -> bool {
    if init().is_err() {
        return false;
    }
    // SAFETY: solo lee el resultado de la sonda hecha en gcv_init.
    unsafe { gcv_op_limit_supported() != 0 }
}

/// Versiones en tiempo de ejecución de libxml2 y libxslt (`20913`, `10135`…).
pub(crate) fn runtime_versions() -> (String, String) {
    // SAFETY: ambas funciones devuelven cadenas estáticas de la biblioteca.
    unsafe {
        (
            CStr::from_ptr(gcv_libxml_version())
                .to_string_lossy()
                .into_owned(),
            CStr::from_ptr(gcv_libxslt_version())
                .to_string_lossy()
                .into_owned(),
        )
    }
}

// ------------------------------------------------------------------ sesión

/// Operación de E/S que libxslt somete a las preferencias de seguridad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IoOperation {
    ReadFile,
    WriteFile,
    CreateDirectory,
    ReadNetwork,
    WriteNetwork,
}

impl IoOperation {
    fn from_code(code: c_int) -> Option<Self> {
        Some(match code {
            1 => Self::ReadFile,
            2 => Self::WriteFile,
            3 => Self::CreateDirectory,
            4 => Self::ReadNetwork,
            5 => Self::WriteNetwork,
            _ => return None,
        })
    }
}

/// Recurso que la política rechazó durante una operación.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Denial {
    pub(crate) operation: IoOperation,
    pub(crate) resource: String,
    pub(crate) reason: &'static str,
}

/// Estado de una operación en curso en este hilo.
#[derive(Default)]
pub(crate) struct Session {
    /// Directorio autorizado (canónico) para lecturas de archivo.
    pub(crate) root: Option<PathBuf>,
    pub(crate) max_file_bytes: u64,
    /// Fragmento pendiente de `xsltGenericError` (sin salto final).
    buffer: Vec<u8>,
    /// Mensajes en el orden de la lista de libxml_get_errors() de la referencia.
    pub(crate) messages: Vec<String>,
    /// Diagnósticos completos, incluidos los que no tienen formato de error.
    pub(crate) runtime_error: bool,
    pub(crate) compile_error: bool,
    pub(crate) op_limit_hit: bool,
    pub(crate) template_depth_hit: bool,
    pub(crate) denials: Vec<Denial>,
    pub(crate) rejected_dtd: Vec<String>,
    pub(crate) extension: Option<ExtensionCall>,
    pub(crate) regex_errors: Vec<xslt_ext::RegexFailure>,
    pub(crate) regex_limits: u32,
    pub(crate) op_limit_code: c_int,
    pub(crate) match_limit: u32,
    pub(crate) depth_limit: u32,
}

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

/// Ejecuta `f` con una sesión activa y los manejadores de error instalados.
/// Devuelve también la sesión final para que el llamador la inspeccione.
fn with_session<T>(mut session: Session, f: impl FnOnce() -> T) -> (T, Session) {
    // SAFETY: gcv_op_limit_code solo devuelve una constante.
    session.op_limit_code = unsafe { gcv_op_limit_code() };
    SESSION.with(|cell| *cell.borrow_mut() = Some(session));
    // SAFETY: push/pop se emparejan en este hilo; los manejadores instalados
    // solo llaman a los callbacks Rust de este módulo.
    let saved = unsafe { gcv_handlers_push() };
    let out = f();
    // SAFETY: `saved` proviene de gcv_handlers_push en este mismo hilo.
    unsafe { gcv_handlers_pop(saved) };
    let session = SESSION
        .with(|cell| cell.borrow_mut().take())
        .unwrap_or_default();
    (out, session)
}

fn session_mut<R>(f: impl FnOnce(&mut Session) -> R) -> Option<R> {
    SESSION.with(|cell| {
        cell.try_borrow_mut()
            .ok()
            .and_then(|mut s| s.as_mut().map(f))
    })
}

fn c_str<'a>(ptr: *const c_char) -> &'a [u8] {
    if ptr.is_null() {
        return &[];
    }
    // SAFETY: glue.c y libxml2 pasan cadenas C terminadas en NUL válidas
    // durante el callback; no se conserva la referencia después.
    unsafe { CStr::from_ptr(ptr) }.to_bytes()
}

/// Reproduce el manejador interno de errores de libxml de la referencia: quita los saltos de línea
/// finales y, si había alguno, publica el búfer acumulado como un mensaje.
fn push_generic(session: &mut Session, chunk: &[u8]) {
    let mut end = chunk.len();
    while end > 0 && chunk[end - 1] == b'\n' {
        end -= 1;
    }
    let flush = end < chunk.len();
    session.buffer.extend_from_slice(&chunk[..end]);
    if flush {
        let text = String::from_utf8_lossy(&session.buffer).into_owned();
        session.buffer.clear();
        if text.starts_with("runtime error") {
            session.runtime_error = true;
        }
        if text.starts_with("compilation error") {
            session.compile_error = true;
        }
        if text.contains("xsltMaxDepth")
            || text.contains("maxTemplateVars")
            || text.contains("xsltMaxVars")
        {
            session.template_depth_hit = true;
        }
        session.messages.push(text);
    }
}

#[unsafe(no_mangle)]
extern "C" fn gcv_rust_generic(msg: *const c_char, len: usize) {
    if msg.is_null() {
        return;
    }
    // SAFETY: glue.c pasa un búfer de `len` bytes recién formateado.
    let chunk = unsafe { std::slice::from_raw_parts(msg.cast::<u8>(), len) };
    session_mut(|s| push_generic(s, chunk));
}

#[unsafe(no_mangle)]
extern "C" fn gcv_rust_structured(_domain: c_int, code: c_int, msg: *const c_char) {
    let text = String::from_utf8_lossy(c_str(msg)).into_owned();
    session_mut(|s| {
        if s.op_limit_code >= 0 && code == s.op_limit_code {
            s.op_limit_hit = true;
        }
        // La referencia añade los errores estructurados sin pasar por el búfer y con
        // su salto de línea final.
        s.messages.push(text);
    });
}

#[unsafe(no_mangle)]
extern "C" fn gcv_rust_active() -> c_int {
    c_int::from(session_mut(|_| ()).is_some())
}

#[unsafe(no_mangle)]
extern "C" fn gcv_rust_check(op: c_int, value: *const c_char) -> c_int {
    let resource = String::from_utf8_lossy(c_str(value)).into_owned();
    let Some(operation) = IoOperation::from_code(op) else {
        return 0;
    };
    // Fuera de una operación del crate (otro usuario de libxslt en el
    // proceso) se mantiene el comportamiento por defecto: permitir.
    session_mut(|s| match check_io(s, operation, &resource) {
        Ok(()) => 1,
        Err(reason) => {
            s.denials.push(Denial {
                operation,
                resource: resource.clone(),
                reason,
            });
            0
        }
    })
    .unwrap_or(1)
}

fn check_io(session: &Session, operation: IoOperation, resource: &str) -> Result<(), &'static str> {
    match operation {
        IoOperation::ReadFile => {}
        IoOperation::ReadNetwork => return Err("acceso de red prohibido"),
        IoOperation::WriteFile | IoOperation::CreateDirectory | IoOperation::WriteNetwork => {
            return Err("escritura prohibida");
        }
    }
    let Some(root) = &session.root else {
        return Err("no hay directorio de reglas autorizado");
    };
    let path = Path::new(resource);
    let canonical = match std::fs::canonicalize(path) {
        Ok(canonical) => canonical,
        // Un archivo inexistente dentro del directorio autorizado se deja
        // fallar a libxslt (error de carga, como en la referencia); fuera, se rechaza.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().and_then(|p| std::fs::canonicalize(p).ok());
            return match parent {
                Some(parent) if parent.starts_with(root) => Ok(()),
                _ => Err("fuera del directorio de reglas"),
            };
        }
        Err(_) => return Err("archivo ilegible"),
    };
    if !canonical.starts_with(root) {
        return Err("fuera del directorio de reglas");
    }
    let meta = std::fs::metadata(&canonical).map_err(|_| "archivo ilegible")?;
    if !meta.is_file() {
        return Err("no es un archivo regular");
    }
    if meta.len() > session.max_file_bytes {
        return Err("supera el tamaño máximo");
    }
    Ok(())
}

#[unsafe(no_mangle)]
extern "C" fn gcv_rust_rejected_dtd(uri: *const c_char) {
    let uri = String::from_utf8_lossy(c_str(uri)).into_owned();
    session_mut(|s| s.rejected_dtd.push(uri));
}

static STR_ONE: &CStr = c"1";
static STR_ZERO: &CStr = c"0";

#[unsafe(no_mangle)]
extern "C" fn gcv_rust_ext_call(
    string_mode: c_int,
    nargs: c_int,
    args: *const RawArg,
    ret: *mut RawRet,
) {
    let count = usize::try_from(nargs).unwrap_or(0);
    let raw: &[RawArg] = if count == 0 || args.is_null() {
        &[]
    } else {
        // SAFETY: glue.c reserva `nargs` elementos inicializados.
        unsafe { std::slice::from_raw_parts(args, count) }
    };
    let values: Vec<xslt_ext::XPathArg> = raw
        .iter()
        .enumerate()
        .map(|(i, a)| match a.kind {
            ARG_STRING => {
                xslt_ext::XPathArg::String(String::from_utf8_lossy(c_str(a.str_)).into_owned())
            }
            ARG_BOOLEAN => xslt_ext::XPathArg::Boolean(a.boolean != 0),
            ARG_NUMBER => xslt_ext::XPathArg::Number(a.num),
            ARG_NODESET => xslt_ext::XPathArg::NodeSet,
            _ if i == 0 => xslt_ext::XPathArg::NotString,
            _ => xslt_ext::XPathArg::String(String::new()),
        })
        .collect();
    let action = session_mut(|s| xslt_ext::dispatch(s, string_mode != 0, &values))
        .unwrap_or(xslt_ext::Push::Sentinel);
    let (code, text, boolean) = match action {
        xslt_ext::Push::Matched(true) => (RET_STRING, STR_ONE.as_ptr(), 0),
        xslt_ext::Push::Matched(false) => (RET_STRING, STR_ZERO.as_ptr(), 0),
        xslt_ext::Push::False => (RET_BOOLEAN, ptr::null(), 0),
        xslt_ext::Push::Nothing => (RET_NOTHING, ptr::null(), 0),
        xslt_ext::Push::Sentinel => (RET_SENTINEL, ptr::null(), 0),
    };
    if ret.is_null() {
        return;
    }
    // SAFETY: `ret` apunta a un gcv_ret de la pila de glue.c.
    unsafe {
        (*ret).action = code;
        (*ret).str_ = text;
        (*ret).boolean = boolean;
    }
}

// --------------------------------------------------------------- documentos

/// Documento libxml2 propio (liberado en `Drop`).
pub(crate) struct Doc(NonNull<XmlDoc>);

// SAFETY: un xmlDoc sin compartir puede moverse entre hilos; libxml2 protege
// con mutex el diccionario compartido. No se implementa Sync.
unsafe impl Send for Doc {}

impl Drop for Doc {
    fn drop(&mut self) {
        // SAFETY: el puntero proviene de gcv_parse y se libera una vez.
        unsafe { gcv_free_doc(self.0.as_ptr()) };
    }
}

/// Resultado de analizar XML con opciones seguras.
pub(crate) enum Parsed {
    Doc(Doc),
    Dtd,
    Malformed(Vec<String>),
}

fn len_c_int(bytes: &[u8]) -> Option<c_int> {
    c_int::try_from(bytes.len()).ok()
}

/// Analiza `bytes` (sin red, sin entidades externas). Rechaza DOCTYPE.
pub(crate) fn parse_xml(bytes: &[u8]) -> Result<Parsed, &'static str> {
    init()?;
    let len = len_c_int(bytes).ok_or("XML demasiado grande para libxml2")?;
    let (doc, session) = with_session(Session::default(), || {
        let mut has_dtd: c_int = 0;
        // SAFETY: buffer y longitud válidos; gcv_parse no conserva el puntero.
        let doc = unsafe { gcv_parse(bytes.as_ptr().cast(), len, ptr::null(), &mut has_dtd) };
        (doc, has_dtd != 0)
    });
    Ok(match doc {
        (_, true) => Parsed::Dtd,
        (doc, false) => match NonNull::new(doc) {
            Some(doc) => Parsed::Doc(Doc(doc)),
            None => Parsed::Malformed(session.messages),
        },
    })
}

impl Doc {
    /// `localName` del elemento raíz.
    pub(crate) fn root_name(&self) -> Option<String> {
        // SAFETY: documento vivo; la cadena pertenece al nodo y se copia ya.
        let name = unsafe { gcv_root_name(self.0.as_ptr()) };
        (!name.is_null()).then(|| String::from_utf8_lossy(c_str(name)).into_owned())
    }

    /// `nodeValue` del primer resultado de `expr` relativo a la raíz, con los
    /// espacios de nombres de la raíz (DOMXPath::query de la referencia). `None` si no
    /// hay nodos o la expresión falla.
    pub(crate) fn root_text(&self, expr: &str) -> Option<String> {
        let expr = CString::new(expr).ok()?;
        let (text, _) = with_session(Session::default(), || {
            let mut out: *mut c_char = ptr::null_mut();
            // SAFETY: documento vivo, expresión C válida; `out` se libera con
            // gcv_free tras copiarlo.
            let status = unsafe { gcv_root_text(self.0.as_ptr(), expr.as_ptr(), &mut out) };
            let text = (status == 0 && !out.is_null())
                .then(|| String::from_utf8_lossy(c_str(out)).into_owned());
            // SAFETY: `out` es NULL o memoria de xmlNodeGetContent.
            unsafe { gcv_free(out.cast()) };
            text
        });
        text
    }
}

// ------------------------------------------------------------------- hojas

/// Hoja XSLT compilada (libera también su documento).
pub(crate) struct Style(NonNull<XsltStylesheet>);

// SAFETY: el puntero solo se usa en `transform` y en `Drop`, ambos con
// ENGINE tomado, así que nunca hay dos hilos operando sobre la hoja a la vez.
unsafe impl Send for Style {}
// SAFETY: ídem; compartir `&Style` solo permite llamar a `transform`.
unsafe impl Sync for Style {}

impl Drop for Style {
    fn drop(&mut self) {
        let _guard = lock_engine();
        // SAFETY: puntero de gcv_compile, liberado una vez.
        unsafe { gcv_free_stylesheet(self.0.as_ptr()) };
    }
}

/// Resultado de compilar una hoja.
pub(crate) enum Compiled {
    Style(Style),
    Dtd,
    Malformed(Session),
    Failed(Session),
}

/// Política de lectura para includes, imports y document().
pub(crate) struct ReadPolicy {
    pub(crate) root: Option<PathBuf>,
    pub(crate) max_file_bytes: u64,
}

/// Compila la hoja `bytes`, cuya URL base es `path` (absoluta).
pub(crate) fn compile_stylesheet(
    bytes: &[u8],
    path: &Path,
    policy: &ReadPolicy,
) -> Result<Compiled, &'static str> {
    init()?;
    let len = len_c_int(bytes).ok_or("XSL demasiado grande para libxml2")?;
    let path_c = CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| "ruta con NUL")?;
    let _guard = lock_engine();
    let session = Session {
        root: policy.root.clone(),
        max_file_bytes: policy.max_file_bytes,
        ..Session::default()
    };
    let (result, session) = with_session(session, || {
        let mut has_dtd: c_int = 0;
        // SAFETY: buffer, longitud y ruta válidos durante la llamada.
        let doc = unsafe {
            gcv_parse_stylesheet_doc(bytes.as_ptr().cast(), len, path_c.as_ptr(), &mut has_dtd)
        };
        if has_dtd != 0 {
            return Err(true);
        }
        if doc.is_null() {
            return Err(false);
        }
        let mut errors: c_int = 0;
        // SAFETY: gcv_compile toma posesión de `doc` (lo libera si falla).
        Ok((unsafe { gcv_compile(doc, &mut errors) }, errors))
    });
    Ok(match result {
        Err(true) => Compiled::Dtd,
        Err(false) => Compiled::Malformed(session),
        Ok((style, errors)) => match NonNull::new(style) {
            Some(style) => {
                let style = Style(style);
                if errors > 0
                    || session.compile_error
                    || !session.denials.is_empty()
                    || !session.rejected_dtd.is_empty()
                {
                    drop_style_locked(style);
                    Compiled::Failed(session)
                } else {
                    Compiled::Style(style)
                }
            }
            None => Compiled::Failed(session),
        },
    })
}

/// Libera una hoja cuando ENGINE ya está tomado por este hilo.
fn drop_style_locked(style: Style) {
    let ptr = style.0.as_ptr();
    std::mem::forget(style);
    // SAFETY: se evita el Drop (que volvería a tomar ENGINE) y se libera una vez.
    unsafe { gcv_free_stylesheet(ptr) };
}

/// Parámetros y límites de una transformación.
pub(crate) struct TransformSpec<'a> {
    pub(crate) filename: &'a CStr,
    pub(crate) op_limit: u64,
    pub(crate) match_limit: u32,
    pub(crate) depth_limit: u32,
}

/// Resultado bruto de una transformación.
pub(crate) struct TransformOutcome {
    pub(crate) session: Session,
    /// 0 OK, 1 ERROR, 2 STOPPED (`xsltTransformState`).
    pub(crate) state: c_int,
}

/// Aplica `style` a una copia de `doc`.
pub(crate) fn transform(
    style: &Style,
    doc: &Doc,
    spec: &TransformSpec<'_>,
    policy: &ReadPolicy,
) -> Result<TransformOutcome, &'static str> {
    init()?;
    let _guard = lock_engine();
    let session = Session {
        root: policy.root.clone(),
        max_file_bytes: policy.max_file_bytes,
        match_limit: spec.match_limit,
        depth_limit: spec.depth_limit,
        ..Session::default()
    };
    let op_limit = c_ulong::try_from(spec.op_limit).unwrap_or(c_ulong::MAX);
    let ((status, state), session) = with_session(session, || {
        let mut state: c_int = 1;
        let mut applied: c_int = 0;
        // SAFETY: hoja y documento vivos (préstamos); el parámetro es una
        // cadena C válida; glue.c trabaja sobre una copia del documento.
        let status = unsafe {
            gcv_transform(
                style.0.as_ptr(),
                doc.0.as_ptr(),
                c"nombreArchivoEnviado".as_ptr(),
                spec.filename.as_ptr(),
                op_limit,
                &mut state,
                &mut applied,
            )
        };
        (status, state)
    });
    if status != 0 {
        return Err("no se pudo preparar la transformación");
    }
    Ok(TransformOutcome { session, state })
}

// -------------------------------------------------------------------- PCRE2

/// Resultado de `pcre2_match` sobre bytes (modo no UTF, como la referencia sin `/u`).
pub(crate) enum PcreResult {
    Matched(bool),
    CompileError { message: String, offset: usize },
    Limit,
    MatchError(i32),
}

/// Compila `pattern` con las opciones de la referencia por defecto y busca en
/// `subject` con los límites `pcre.backtrack_limit`/`pcre.recursion_limit`.
pub(crate) fn pcre_match(
    pattern: &[u8],
    subject: &[u8],
    match_limit: u32,
    depth_limit: u32,
) -> PcreResult {
    use pcre2_sys as p;
    // SAFETY: todos los punteros PCRE2 se crean y liberan aquí; los búferes
    // de patrón y sujeto viven durante las llamadas.
    unsafe {
        let cctx = p::pcre2_compile_context_create_8(ptr::null_mut());
        if cctx.is_null() {
            return PcreResult::MatchError(p::PCRE2_ERROR_NOMEMORY);
        }
        p::pcre2_set_compile_extra_options_8(cctx, p::PCRE2_EXTRA_ALLOW_LOOKAROUND_BSK);
        let mut errcode: c_int = 0;
        let mut erroffset: usize = 0;
        let code = p::pcre2_compile_8(
            pattern.as_ptr(),
            pattern.len(),
            0,
            &mut errcode,
            &mut erroffset,
            cctx,
        );
        p::pcre2_compile_context_free_8(cctx);
        if code.is_null() {
            let mut buf = [0u8; 256];
            let n = p::pcre2_get_error_message_8(errcode, buf.as_mut_ptr(), buf.len());
            let len = usize::try_from(n).unwrap_or(0).min(buf.len());
            return PcreResult::CompileError {
                message: String::from_utf8_lossy(&buf[..len]).into_owned(),
                offset: erroffset,
            };
        }
        let mctx = p::pcre2_match_context_create_8(ptr::null_mut());
        let md = p::pcre2_match_data_create_from_pattern_8(code, ptr::null_mut());
        if mctx.is_null() || md.is_null() {
            if !mctx.is_null() {
                p::pcre2_match_context_free_8(mctx);
            }
            if !md.is_null() {
                p::pcre2_match_data_free_8(md);
            }
            p::pcre2_code_free_8(code);
            return PcreResult::MatchError(p::PCRE2_ERROR_NOMEMORY);
        }
        p::pcre2_set_match_limit_8(mctx, match_limit);
        p::pcre2_set_depth_limit_8(mctx, depth_limit);
        let rc = p::pcre2_match_8(
            code,
            subject.as_ptr(),
            subject.len(),
            0,
            p::PCRE2_NO_UTF_CHECK,
            md,
            mctx,
        );
        p::pcre2_match_data_free_8(md);
        p::pcre2_match_context_free_8(mctx);
        p::pcre2_code_free_8(code);
        match rc {
            n if n >= 0 => PcreResult::Matched(true),
            p::PCRE2_ERROR_NOMATCH => PcreResult::Matched(false),
            p::PCRE2_ERROR_MATCHLIMIT
            | p::PCRE2_ERROR_DEPTHLIMIT
            | p::PCRE2_ERROR_HEAPLIMIT
            | p::PCRE2_ERROR_JIT_STACKLIMIT => PcreResult::Limit,
            other => PcreResult::MatchError(other),
        }
    }
}
