/*
 * Puente mínimo entre cpeforge-cpe-validator y libxml2/libxslt/libexslt.
 *
 * Se escribe en C, compilado contra las cabeceras reales del sistema, por
 * dos motivos:
 *  1. libxslt entrega xsl:message y sus errores mediante callbacks
 *     variádicos (xmlGenericErrorFunc), que Rust estable no puede definir.
 *  2. El estado de la transformación (ctxt->state), el contexto XPath del
 *     espacio de nombres y el límite de operaciones XPath son campos de
 *     estructuras C: leerlos aquí evita adivinar su disposición desde Rust.
 *
 * El resto (formato de mensajes, permisos de lectura, php:function, PCRE)
 * vive en Rust, que recibe callbacks no variádicos. Todas las funciones
 * exportadas usan el prefijo gcv_.
 */

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libxml/parser.h>
#include <libxml/tree.h>
#include <libxml/uri.h>
#include <libxml/xmlerror.h>
#include <libxml/xmlversion.h>
#include <libxml/xpath.h>
#include <libxml/xpathInternals.h>
#include <libxslt/documents.h>
#include <libxslt/extensions.h>
#include <libxslt/security.h>
#include <libxslt/transform.h>
#include <libxslt/variables.h>
#include <libxslt/xslt.h>
#include <libxslt/xsltInternals.h>
#include <libxslt/xsltutils.h>
#include <libexslt/exslt.h>

/* Callbacks implementados en Rust (src/ffi.rs). */
extern void gcv_rust_generic(const char *msg, size_t len);
extern void gcv_rust_structured(int domain, int code, const char *msg);
extern int gcv_rust_active(void);
extern int gcv_rust_check(int op, const char *value);
extern void gcv_rust_rejected_dtd(const char *uri);

typedef struct {
    int kind;          /* GCV_ARG_* */
    const char *str;   /* válido durante la llamada */
    double num;
    int boolean;
} gcv_arg;

typedef struct {
    int action;        /* GCV_RET_* */
    const char *str;   /* cadena estática de Rust para GCV_RET_STRING */
    int boolean;
} gcv_ret;

extern void gcv_rust_ext_call(int string_mode, int nargs, const gcv_arg *args, gcv_ret *ret);

enum { GCV_ARG_STRING = 0, GCV_ARG_BOOLEAN = 1, GCV_ARG_NUMBER = 2, GCV_ARG_NODESET = 3, GCV_ARG_NOT_STRING = 4 };
enum { GCV_RET_STRING = 0, GCV_RET_BOOLEAN = 1, GCV_RET_NOTHING = 2, GCV_RET_SENTINEL = 3 };
enum { GCV_READ_FILE = 1, GCV_WRITE_FILE = 2, GCV_CREATE_DIRECTORY = 3, GCV_READ_NETWORK = 4, GCV_WRITE_NETWORK = 5 };

#if LIBXML_VERSION >= 21200
#define GCV_CONST_ERROR const xmlError *
#else
#define GCV_CONST_ERROR xmlErrorPtr
#endif

/* Campos opLimit/opCount de xmlXPathContext: la libxml2 de Apple los declara
 * solo con LIBXML_HAS_XPATH_RESOURCE_LIMITS; la libxml2 estándar los tiene
 * siempre desde 2.9.11. Si el límite se aplica de verdad lo decide la sonda
 * gcv_probe_op_limit en tiempo de ejecución. */
#if defined(LIBXML_HAS_XPATH_RESOURCE_LIMITS) || (!defined(__APPLE__) && LIBXML_VERSION >= 20911)
#define GCV_XPATH_OP_LIMIT 1
#endif

/* ---------------------------------------------------------------- errores */

/* Formatea un mensaje printf en memoria nueva (liberar con free). */
static char *gcv_vformat(const char *fmt, va_list ap, size_t *len)
{
    va_list copy;
    int needed;
    char *buf;

    va_copy(copy, ap);
    needed = vsnprintf(NULL, 0, fmt, copy);
    va_end(copy);
    if (needed < 0) {
        return NULL;
    }
    buf = malloc((size_t) needed + 1);
    if (buf == NULL) {
        return NULL;
    }
    vsnprintf(buf, (size_t) needed + 1, fmt, ap);
    *len = (size_t) needed;
    return buf;
}

static void gcv_generic_error(void *ctx, const char *fmt, ...)
{
    va_list ap;
    size_t len = 0;
    char *buf;
    (void) ctx;

    va_start(ap, fmt);
    buf = gcv_vformat(fmt, ap, &len);
    va_end(ap);
    if (buf != NULL) {
        gcv_rust_generic(buf, len);
        free(buf);
    }
}

/* xsltGenericError es una variable global del proceso, no del hilo: si cada
 * operación la instalara y restaurara, un hilo que termina podría quitar el
 * manejador mientras otro transforma y sus xsl:message se perderían. Se
 * instala un enrutador que entrega cada mensaje a la sesión Rust del hilo que
 * lo produce y, sin sesión activa, al manejador que hubiera antes. */
static xmlGenericErrorFunc gcv_host_xslt_error = NULL;
static void *gcv_host_xslt_error_ctx = NULL;

static void gcv_xslt_error(void *ctx, const char *fmt, ...)
{
    va_list ap;
    size_t len = 0;
    char *buf;
    (void) ctx;

    va_start(ap, fmt);
    buf = gcv_vformat(fmt, ap, &len);
    va_end(ap);
    if (buf == NULL) {
        return;
    }
    if (gcv_rust_active()) {
        gcv_rust_generic(buf, len);
    } else if (gcv_host_xslt_error != NULL) {
        gcv_host_xslt_error(gcv_host_xslt_error_ctx, "%s", buf);
    }
    free(buf);
}

/* Instala el enrutador si otro código lo sustituyó. Se llama con el candado
 * ENGINE de Rust tomado (compilación y transformación) y desde gcv_init. */
static void gcv_route_xslt_errors(void)
{
    if (xsltGenericError != gcv_xslt_error) {
        gcv_host_xslt_error = xsltGenericError;
        gcv_host_xslt_error_ctx = xsltGenericErrorContext;
        xsltSetGenericErrorFunc(NULL, gcv_xslt_error);
    }
}

static void gcv_structured_error(void *ctx, GCV_CONST_ERROR error)
{
    (void) ctx;
    if (error != NULL) {
        gcv_rust_structured(error->domain, error->code, error->message != NULL ? error->message : "");
    }
}

typedef struct {
    xmlGenericErrorFunc generic;
    void *generic_ctx;
    xmlStructuredErrorFunc structured;
    void *structured_ctx;
} gcv_saved_handlers;

/* Instala los manejadores propios de libxml2 en este hilo (son por hilo) y
 * devuelve los anteriores. Los mensajes de libxslt los reparte
 * gcv_xslt_error. */
void *gcv_handlers_push(void)
{
    gcv_saved_handlers *saved = malloc(sizeof(gcv_saved_handlers));
    if (saved == NULL) {
        return NULL;
    }
    saved->generic = xmlGenericError;
    saved->generic_ctx = xmlGenericErrorContext;
    saved->structured = xmlStructuredError;
    saved->structured_ctx = xmlStructuredErrorContext;
    xmlSetGenericErrorFunc(NULL, gcv_generic_error);
    xmlSetStructuredErrorFunc(NULL, (xmlStructuredErrorFunc) gcv_structured_error);
    return saved;
}

void gcv_handlers_pop(void *opaque)
{
    gcv_saved_handlers *saved = opaque;
    if (saved == NULL) {
        return;
    }
    xmlSetGenericErrorFunc(saved->generic_ctx, saved->generic);
    xmlSetStructuredErrorFunc(saved->structured_ctx, saved->structured);
    free(saved);
}

/* ------------------------------------------------------------- seguridad */

static int gcv_sec_read_file(xsltSecurityPrefsPtr sec, xsltTransformContextPtr ctxt, const char *value)
{
    (void) sec; (void) ctxt;
    return gcv_rust_check(GCV_READ_FILE, value);
}

static int gcv_sec_write_file(xsltSecurityPrefsPtr sec, xsltTransformContextPtr ctxt, const char *value)
{
    (void) sec; (void) ctxt;
    return gcv_rust_check(GCV_WRITE_FILE, value);
}

static int gcv_sec_create_directory(xsltSecurityPrefsPtr sec, xsltTransformContextPtr ctxt, const char *value)
{
    (void) sec; (void) ctxt;
    return gcv_rust_check(GCV_CREATE_DIRECTORY, value);
}

static int gcv_sec_read_network(xsltSecurityPrefsPtr sec, xsltTransformContextPtr ctxt, const char *value)
{
    (void) sec; (void) ctxt;
    return gcv_rust_check(GCV_READ_NETWORK, value);
}

static int gcv_sec_write_network(xsltSecurityPrefsPtr sec, xsltTransformContextPtr ctxt, const char *value)
{
    (void) sec; (void) ctxt;
    return gcv_rust_check(GCV_WRITE_NETWORK, value);
}

static xsltSecurityPrefsPtr gcv_new_prefs(void)
{
    xsltSecurityPrefsPtr sec = xsltNewSecurityPrefs();
    if (sec == NULL) {
        return NULL;
    }
    if (xsltSetSecurityPrefs(sec, XSLT_SECPREF_READ_FILE, gcv_sec_read_file) != 0
        || xsltSetSecurityPrefs(sec, XSLT_SECPREF_WRITE_FILE, gcv_sec_write_file) != 0
        || xsltSetSecurityPrefs(sec, XSLT_SECPREF_CREATE_DIRECTORY, gcv_sec_create_directory) != 0
        || xsltSetSecurityPrefs(sec, XSLT_SECPREF_READ_NETWORK, gcv_sec_read_network) != 0
        || xsltSetSecurityPrefs(sec, XSLT_SECPREF_WRITE_NETWORK, gcv_sec_write_network) != 0) {
        xsltFreeSecurityPrefs(sec);
        return NULL;
    }
    return sec;
}

/* Opciones de análisis seguras: sin red, sin sustituir entidades, sin
 * cargar DTD externos ni XInclude. */
static int gcv_safe_options(int options)
{
    options &= ~(XML_PARSE_NOENT | XML_PARSE_DTDLOAD | XML_PARSE_DTDATTR | XML_PARSE_DTDVALID | XML_PARSE_XINCLUDE);
    options |= XML_PARSE_NONET;
#if LIBXML_VERSION >= 21300
    options |= XML_PARSE_NO_XXE;
#endif
    return options;
}

static xsltDocLoaderFunc gcv_original_loader = NULL;
static int gcv_op_limit_state = -1;
static int gcv_probe_op_limit(void);

/* Cargador de xsl:include, xsl:import y document(): libxslt ya comprobó
 * los permisos; aquí se fuerzan opciones seguras y se rechaza cualquier DTD.
 * Fuera de una operación de este crate se delega sin cambios. */
static xmlDocPtr gcv_loader(const xmlChar *uri, xmlDictPtr dict, int options, void *ctxt, xsltLoadType type)
{
    xmlDocPtr doc;
    if (gcv_original_loader == NULL) {
        return NULL;
    }
    if (!gcv_rust_active()) {
        return gcv_original_loader(uri, dict, options, ctxt, type);
    }
    doc = gcv_original_loader(uri, dict, gcv_safe_options(options), ctxt, type);
    if (doc != NULL && xmlGetIntSubset(doc) != NULL) {
        gcv_rust_rejected_dtd((const char *) uri);
        xmlFreeDoc(doc);
        return NULL;
    }
    return doc;
}

/* Inicialización única (protegida por std::sync::Once en Rust). */
int gcv_init(void)
{
    xsltSecurityPrefsPtr defaults;
    xmlInitParser();
    exsltRegisterAll();
    gcv_route_xslt_errors();
    if (gcv_op_limit_state < 0) {
        gcv_op_limit_state = gcv_probe_op_limit();
    }
    if (gcv_original_loader == NULL) {
        gcv_original_loader = xsltDocDefaultLoader;
        xsltSetLoaderFunc(gcv_loader);
    }
    /* xsl:include/xsl:import consultan las preferencias por defecto. */
    defaults = gcv_new_prefs();
    if (defaults == NULL) {
        return -1;
    }
    xsltSetDefaultSecurityPrefs(defaults);
    return 0;
}

const char *gcv_libxml_version(void)
{
    return xmlParserVersion;
}

const char *gcv_libxslt_version(void)
{
    return xsltEngineVersion;
}

/* ------------------------------------------------------------- documentos */

/* Analiza XML con opciones seguras. *has_dtd indica una declaración DOCTYPE
 * (el documento se libera y se devuelve NULL). */
xmlDocPtr gcv_parse(const char *buf, int len, const char *url, int *has_dtd)
{
    xmlDocPtr doc = xmlReadMemory(buf, len, url, NULL, gcv_safe_options(0));
    *has_dtd = 0;
    if (doc != NULL && xmlGetIntSubset(doc) != NULL) {
        *has_dtd = 1;
        xmlFreeDoc(doc);
        return NULL;
    }
    return doc;
}

void gcv_free_doc(xmlDocPtr doc)
{
    if (doc != NULL) {
        xmlFreeDoc(doc);
    }
}

void gcv_free(void *ptr)
{
    if (ptr != NULL) {
        xmlFree(ptr);
    }
}

/* Nombre del elemento raíz tal como lo expone DOMNode::$localName de la referencia
 * (node->name; con un prefijo no declarado incluye "prefijo:"). */
const char *gcv_root_name(xmlDocPtr doc)
{
    xmlNodePtr root = xmlDocGetRootElement(doc);
    return root != NULL ? (const char *) root->name : NULL;
}

/* Equivalente a DOMXPath::query($expr, $doc->documentElement) con
 * registerNodeNS=true y nodeValue del primer nodo.
 * Devuelve 0 si hay resultado (*out con xmlFree), 1 si el conjunto está
 * vacío y 2 si la expresión falla (p. ej. prefijo sin declarar). */
int gcv_root_text(xmlDocPtr doc, const char *expr, char **out)
{
    xmlNodePtr root = xmlDocGetRootElement(doc);
    xmlXPathContextPtr ctx;
    xmlXPathObjectPtr result;
    xmlNsPtr *ns;
    int status = 1;

    *out = NULL;
    if (root == NULL) {
        return 2;
    }
    ctx = xmlXPathNewContext(doc);
    if (ctx == NULL) {
        return 2;
    }
    ctx->node = root;
    ns = xmlGetNsList(doc, root);
    if (ns != NULL) {
        int count = 0;
        while (ns[count] != NULL) {
            count++;
        }
        ctx->namespaces = ns;
        ctx->nsNr = count;
    }
    result = xmlXPathEvalExpression((const xmlChar *) expr, ctx);
    if (result == NULL) {
        status = 2;
    } else if (result->type == XPATH_NODESET && result->nodesetval != NULL && result->nodesetval->nodeNr > 0) {
        *out = (char *) xmlNodeGetContent(result->nodesetval->nodeTab[0]);
        status = *out != NULL ? 0 : 1;
    } else if (result->type != XPATH_NODESET) {
        status = 2;
    }
    if (result != NULL) {
        xmlXPathFreeObject(result);
    }
    ctx->namespaces = NULL;
    ctx->nsNr = 0;
    if (ns != NULL) {
        xmlFree(ns);
    }
    xmlXPathFreeContext(ctx);
    return status;
}

/* --------------------------------------------------------------- hojas XSL */

/* Compila una hoja. Toma posesión de doc: en caso de error lo libera.
 * *errors recibe style->errors cuando la compilación devuelve una hoja. */
xsltStylesheetPtr gcv_compile(xmlDocPtr doc, int *errors)
{
    xsltStylesheetPtr style;
    gcv_route_xslt_errors();
    style = xsltParseStylesheetDoc(doc);
    *errors = 0;
    if (style == NULL) {
        xmlFreeDoc(doc);
        return NULL;
    }
    *errors = style->errors;
    return style;
}

void gcv_free_stylesheet(xsltStylesheetPtr style)
{
    if (style != NULL) {
        xsltFreeStylesheet(style);
    }
}

/* Documento con la URL que la referencia asigna al importar la hoja
 * (xmlNodeSetBase(newdoc, doc->URL) → xmlPathToURI). */
xmlDocPtr gcv_parse_stylesheet_doc(const char *buf, int len, const char *path, int *has_dtd)
{
    xmlChar *uri = xmlPathToURI((const xmlChar *) path);
    xmlDocPtr doc = gcv_parse(buf, len, uri != NULL ? (const char *) uri : path, has_dtd);
    if (uri != NULL) {
        xmlFree(uri);
    }
    return doc;
}

/* ------------------------------------------------------------ php:function */

static void gcv_ext_dispatch(xmlXPathParserContextPtr ctxt, int nargs, int string_mode)
{
    gcv_arg *args = NULL;
    xmlXPathObjectPtr *objs = NULL;
    xmlChar **owned = NULL;
    gcv_ret ret = { GCV_RET_SENTINEL, NULL, 0 };
    int i;

    if (nargs > 0) {
        args = calloc((size_t) nargs, sizeof(gcv_arg));
        objs = calloc((size_t) nargs, sizeof(xmlXPathObjectPtr));
        owned = calloc((size_t) nargs, sizeof(xmlChar *));
        if (args == NULL || objs == NULL || owned == NULL) {
            free(args);
            free(objs);
            free(owned);
            xmlXPathErr(ctxt, XPATH_MEMORY_ERROR);
            return;
        }
        /* La pila tiene el nombre al fondo y el último argumento arriba. */
        for (i = nargs - 1; i >= 0; i--) {
            objs[i] = valuePop(ctxt);
        }
        for (i = 0; i < nargs; i++) {
            xmlXPathObjectPtr obj = objs[i];
            gcv_arg *arg = &args[i];
            if (obj == NULL) {
                arg->kind = GCV_ARG_NOT_STRING;
                continue;
            }
            if (i == 0) {
                /* la referencia exige obj->stringval: solo una cadena XPath lo tiene. */
                if (obj->type == XPATH_STRING && obj->stringval != NULL) {
                    arg->kind = GCV_ARG_STRING;
                    arg->str = (const char *) obj->stringval;
                } else {
                    arg->kind = GCV_ARG_NOT_STRING;
                }
                continue;
            }
            switch (obj->type) {
                case XPATH_STRING:
                    arg->kind = GCV_ARG_STRING;
                    arg->str = obj->stringval != NULL ? (const char *) obj->stringval : "";
                    break;
                case XPATH_BOOLEAN:
                    arg->kind = GCV_ARG_BOOLEAN;
                    arg->boolean = obj->boolval;
                    break;
                case XPATH_NUMBER:
                    arg->kind = GCV_ARG_NUMBER;
                    arg->num = obj->floatval;
                    break;
                case XPATH_NODESET:
                    if (!string_mode) {
                        arg->kind = GCV_ARG_NODESET;
                        break;
                    }
                    /* fallthrough: php:functionString convierte a cadena */
                default: {
                    xmlChar *str = xmlXPathCastToString(obj);
                    owned[i] = str;
                    arg->kind = GCV_ARG_STRING;
                    arg->str = str != NULL ? (const char *) str : "";
                    break;
                }
            }
        }
    }

    gcv_rust_ext_call(string_mode, nargs, args, &ret);

    for (i = 0; i < nargs; i++) {
        if (objs[i] != NULL) {
            xmlXPathFreeObject(objs[i]);
        }
        if (owned[i] != NULL) {
            xmlFree(owned[i]);
        }
    }
    free(args);
    free(objs);
    free(owned);

    switch (ret.action) {
        case GCV_RET_STRING:
            valuePush(ctxt, xmlXPathNewString((const xmlChar *) (ret.str != NULL ? ret.str : "")));
            break;
        case GCV_RET_BOOLEAN:
            valuePush(ctxt, xmlXPathNewBoolean(ret.boolean));
            break;
        case GCV_RET_NOTHING:
            break;
        default:
            valuePush(ctxt, xmlXPathNewString((const xmlChar *) ""));
            break;
    }
}

static void gcv_ext_function(xmlXPathParserContextPtr ctxt, int nargs)
{
    gcv_ext_dispatch(ctxt, nargs, 0);
}

static void gcv_ext_function_string(xmlXPathParserContextPtr ctxt, int nargs)
{
    gcv_ext_dispatch(ctxt, nargs, 1);
}

/* ---------------------------------------------------------- transformación */

#if defined(GCV_XPATH_OP_LIMIT)
static void gcv_silent_error(void *ctx, GCV_CONST_ERROR error)
{
    (void) ctx; (void) error;
}
#endif

/* gcv_op_limit_state: -1 sin sondear, 0 no, 1 sí; lo fija gcv_init. */

/* Los campos opLimit/opCount existen desde libxml2 2.9.11, pero la libxml2
 * de Apple solo los aplica a binarios enlazados para macOS >= 13
 * («linked-on-or-after»). Se comprueba ejecutando una expresión con límite 1. */
static int gcv_probe_op_limit(void)
{
#if defined(GCV_XPATH_OP_LIMIT)
    static const char probe[] = "<a><b/><b/><b/></a>";
    xmlDocPtr doc;
    xmlXPathContextPtr ctx;
    xmlXPathObjectPtr result;
    xmlStructuredErrorFunc old = xmlStructuredError;
    void *old_ctx = xmlStructuredErrorContext;
    int limited = 0;

    if (atoi(xmlParserVersion) < 20911) {
        return 0;
    }
    doc = xmlReadMemory(probe, (int) sizeof(probe) - 1, NULL, NULL, XML_PARSE_NONET);
    if (doc == NULL) {
        return 0;
    }
    ctx = xmlXPathNewContext(doc);
    if (ctx != NULL) {
        xmlSetStructuredErrorFunc(NULL, (xmlStructuredErrorFunc) gcv_silent_error);
        ctx->opLimit = 1;
        result = xmlXPathEvalExpression((const xmlChar *) "count(//*//*)", ctx);
        limited = result == NULL && ctx->opCount >= 1;
        if (result != NULL) {
            xmlXPathFreeObject(result);
        }
        xmlSetStructuredErrorFunc(old_ctx, old);
        xmlXPathFreeContext(ctx);
    }
    xmlFreeDoc(doc);
    return limited;
#else
    return 0;
#endif
}

static int gcv_runtime_supports_op_limit(void)
{
    return gcv_op_limit_state == 1;
}

int gcv_op_limit_supported(void)
{
    return gcv_runtime_supports_op_limit();
}

int gcv_op_limit_code(void)
{
#if defined(GCV_XPATH_OP_LIMIT)
    /* xmlXPathErr publica code = XML_XPATH_EXPRESSION_OK + error. */
    return XML_XPATH_EXPRESSION_OK + XPATH_OP_LIMIT_EXCEEDED;
#else
    return -1;
#endif
}

/* Aplica la hoja a una copia de src (libxslt puede modificar la fuente al
 * eliminar espacios). Devuelve 0 si se ejecutó, -1 si falló la preparación.
 * *state recibe ctxt->state (0 OK, 1 ERROR, 2 STOPPED). */
int gcv_transform(xsltStylesheetPtr style, xmlDocPtr src, const char *param_name, const char *param_value,
                  unsigned long op_limit, int *state, int *limit_applied)
{
    xmlDocPtr copy;
    xsltTransformContextPtr ctxt;
    xsltSecurityPrefsPtr sec;
    xmlDocPtr result;
    int status = 0;

    *state = XSLT_STATE_ERROR;
    *limit_applied = 0;
    gcv_route_xslt_errors();
    copy = xmlCopyDoc(src, 1);
    if (copy == NULL) {
        return -1;
    }
    ctxt = xsltNewTransformContext(style, copy);
    if (ctxt == NULL) {
        xmlFreeDoc(copy);
        return -1;
    }
    /* El valor por defecto es global (xsltSetXIncludeDefault): se fija aquí
     * para que otro código del proceso no active XInclude en las fuentes. */
    ctxt->xinclude = 0;
    sec = gcv_new_prefs();
    if (sec == NULL || xsltSetCtxtSecurityPrefs(sec, ctxt) != 0) {
        status = -1;
        goto done;
    }
    if (xsltRegisterExtFunction(ctxt, (const xmlChar *) "function", (const xmlChar *) "http://php.net/xsl", gcv_ext_function) != 0
        || xsltRegisterExtFunction(ctxt, (const xmlChar *) "functionString", (const xmlChar *) "http://php.net/xsl", gcv_ext_function_string) != 0) {
        status = -1;
        goto done;
    }
    if (param_name != NULL && xsltQuoteOneUserParam(ctxt, (const xmlChar *) param_name, (const xmlChar *) param_value) != 0) {
        status = -1;
        goto done;
    }
#if defined(GCV_XPATH_OP_LIMIT)
    if (op_limit != 0 && ctxt->xpathCtxt != NULL && gcv_runtime_supports_op_limit()) {
        ctxt->xpathCtxt->opLimit = op_limit;
        ctxt->xpathCtxt->opCount = 0;
        *limit_applied = 1;
    }
#else
    (void) op_limit;
#endif
    result = xsltApplyStylesheetUser(style, copy, NULL, NULL, NULL, ctxt);
    *state = (int) ctxt->state;
    if (result != NULL) {
        xmlFreeDoc(result);
    }
done:
    xsltFreeTransformContext(ctxt);
    if (sec != NULL) {
        xsltFreeSecurityPrefs(sec);
    }
    xmlFreeDoc(copy);
    return status;
}
