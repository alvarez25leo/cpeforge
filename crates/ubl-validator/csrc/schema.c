#include <libxml/parser.h>
#include <libxml/xmlversion.h>
#include <libxml/xmlschemas.h>
#include <string.h>

/* libxml2 2.12 declara const el error de xmlStructuredErrorFunc. */
#if LIBXML_VERSION >= 21200
#define CPEFORGE_CONST_ERROR const xmlError *
#else
#define CPEFORGE_CONST_ERROR xmlErrorPtr
#endif

typedef void (*cpeforge_error_cb)(void *, int, int, int, int, const char *);
typedef struct { cpeforge_error_cb callback; void *user; } cpeforge_sink;

/* Inicializa libxml2. Antes de 2.11 xmlInitParser no es seguro si dos hilos
 * lo llaman a la vez; el lado Rust lo invoca una sola vez (std::sync::Once)
 * antes de cualquier otra función de este archivo. */
void cpeforge_init(void) {
    xmlInitParser();
}

/* Conserva la semántica de codificaciones XML de libxml2. No usa NOENT ni
 * DTDLOAD: la declaración se inspecciona antes de usar el documento. */
int cpeforge_preflight_xml(const unsigned char *xml, int length,
                          int max_nodes, int max_depth) {
    xmlDocPtr doc = xmlReadMemory((const char *)xml, length, NULL, NULL,
                                  XML_PARSE_NONET | XML_PARSE_COMPACT |
                                  XML_PARSE_NOERROR | XML_PARSE_NOWARNING);
    if (!doc) return 0;
    if (doc->intSubset || doc->extSubset) {
        xmlFreeDoc(doc);
        return -1;
    }
    int nodes = 0;
    int depth = 0;
    xmlNodePtr node = doc->children;
    while (node) {
        if (++nodes > max_nodes || depth > max_depth) {
            xmlFreeDoc(doc);
            return -2;
        }
        if (node->children) {
            node = node->children;
            ++depth;
        } else {
            while (node && !node->next) {
                node = node->parent;
                --depth;
                if (node && node->type == XML_DOCUMENT_NODE) node = NULL;
            }
            if (node) node = node->next;
        }
    }
    xmlFreeDoc(doc);
    return 1;
}

static void report(void *ctx, CPEFORGE_CONST_ERROR error) {
    cpeforge_sink *sink = (cpeforge_sink *)ctx;
    if (sink && sink->callback && error) {
        sink->callback(sink->user, error->level, error->code,
                       error->int2, error->line, error->message ? error->message : "");
    }
}

/* Parsea los bytes según su declaración XML (incluye ISO-8859-1 y CP1252).
 * DOMXPath en la referencia autoregistra el prefijo cbc de la raíz y éste prevalece
 * sobre el registro explícito de UblVersionResolver. */
int cpeforge_probe_xml(const unsigned char *xml, int length,
                       char *name, int name_cap, char *version, int version_cap) {
    xmlDocPtr doc = xmlReadMemory((const char *)xml, length, NULL, NULL,
                                  XML_PARSE_NONET | XML_PARSE_COMPACT |
                                  XML_PARSE_NOERROR | XML_PARSE_NOWARNING);
    if (!doc) return 0;
    if (doc->intSubset || doc->extSubset) { xmlFreeDoc(doc); return -2; }
    xmlNodePtr root = xmlDocGetRootElement(doc);
    if (!root || !root->name) { xmlFreeDoc(doc); return 0; }
    const xmlChar *cbc = BAD_CAST "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";
    for (xmlNsPtr ns = root->nsDef; ns; ns = ns->next) {
        if (ns->prefix && xmlStrEqual(ns->prefix, BAD_CAST "cbc")) {
            cbc = ns->href;
            break;
        }
    }
    xmlChar *value = NULL;
    for (xmlNodePtr child = root->children; child; child = child->next) {
        if (child->type == XML_ELEMENT_NODE && child->ns &&
            xmlStrEqual(child->name, BAD_CAST "UBLVersionID") &&
            xmlStrEqual(child->ns->href, cbc)) {
            value = xmlNodeGetContent(child);
            break;
        }
    }
    size_t name_length = xmlStrlen(root->name);
    size_t version_length = value ? xmlStrlen(value) : 0;
    int result = -1;
    if (name_length < (size_t)name_cap && version_length < (size_t)version_cap) {
        memcpy(name, root->name, name_length);
        name[name_length] = 0;
        if (value) memcpy(version, value, version_length);
        version[version_length] = 0;
        result = 1;
    }
    if (value) xmlFree(value);
    xmlFreeDoc(doc);
    return result;
}

/* No global libxml error/entity handler is changed. The caller verifies every
 * reachable schemaLocation and SHA-256 before this function is called. */
int cpeforge_schema_validate(const unsigned char *xml, int length,
                             const char *schema_path, cpeforge_error_cb callback,
                             void *user) {
    cpeforge_sink sink = {callback, user};
    xmlDocPtr doc = xmlReadMemory((const char *)xml, length, NULL, NULL,
                                  XML_PARSE_NONET | XML_PARSE_COMPACT |
                                  XML_PARSE_NOERROR | XML_PARSE_NOWARNING);
    if (!doc) return -1;
    if (doc->intSubset || doc->extSubset) { xmlFreeDoc(doc); return -4; }
    xmlSchemaParserCtxtPtr parser = xmlSchemaNewParserCtxt(schema_path);
    if (!parser) { xmlFreeDoc(doc); return -2; }
    xmlSchemaSetParserStructuredErrors(parser, report, &sink);
    xmlSchemaPtr schema = xmlSchemaParse(parser);
    xmlSchemaFreeParserCtxt(parser);
    if (!schema) { xmlFreeDoc(doc); return -2; }
    xmlSchemaValidCtxtPtr validator = xmlSchemaNewValidCtxt(schema);
    if (!validator) { xmlSchemaFree(schema); xmlFreeDoc(doc); return -2; }
    xmlSchemaSetValidStructuredErrors(validator, report, &sink);
    int result = xmlSchemaValidateDoc(validator, doc);
    xmlSchemaFreeValidCtxt(validator);
    xmlSchemaFree(schema);
    xmlFreeDoc(doc);
    return result == 0 ? 1 : result > 0 ? 0 : -3;
}
