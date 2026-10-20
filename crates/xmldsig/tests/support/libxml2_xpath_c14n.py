"""Oráculo independiente de la transformación XPath de XMLDSig con libxml2.

Lee de stdin un JSON con una lista de casos y escribe en stdout otra lista,
en el mismo orden, con la salida canónica en Base64 o el error. Usa la
libxml2 del sistema mediante ctypes (sin dependencias de Python): el conjunto
de nodos es `(.//. | .//@* | .//namespace::*)[expr]` evaluado desde el nodo
de contexto (la consulta que construye el crate, igual que la referencia) y
se canonicaliza con `xmlC14NDocDumpMemory`, como hace xmlsec. No lee archivos ni red: la entrada se analiza con XML_PARSE_NONET.

Caso: {"xml", "context" (null = documento, o una expresión XPath que elige
el elemento), "expr", "namespaces": [[prefijo, uri]...], "mode"
("c14n" | "exc-c14n"), "with_comments", "inclusive_prefixes": [...]}.
"""

import base64
import ctypes
import ctypes.util
import json
import sys

XML_PARSE_NONET = 1 << 11
MODES = {"c14n": 0, "exc-c14n": 1}


def load():
    for name in (ctypes.util.find_library("xml2"), "libxml2.so.2", "/usr/lib/libxml2.dylib"):
        if not name:
            continue
        try:
            return ctypes.CDLL(name)
        except OSError:
            continue
    raise SystemExit("libxml2 no disponible")


LIB = load()
P = ctypes.c_void_p
LIB.xmlReadMemory.restype = P
LIB.xmlReadMemory.argtypes = [ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int]
LIB.xmlXPathNewContext.restype = P
LIB.xmlXPathNewContext.argtypes = [P]
LIB.xmlXPathRegisterNs.restype = ctypes.c_int
LIB.xmlXPathRegisterNs.argtypes = [P, ctypes.c_char_p, ctypes.c_char_p]
LIB.xmlXPathNodeEval.restype = P
LIB.xmlXPathNodeEval.argtypes = [P, ctypes.c_char_p, P]
LIB.xmlC14NDocDumpMemory.restype = ctypes.c_int
LIB.xmlC14NDocDumpMemory.argtypes = [
    P, P, ctypes.c_int, ctypes.POINTER(ctypes.c_char_p), ctypes.c_int, ctypes.POINTER(ctypes.c_char_p)
]
LIB.xmlXPathFreeObject.argtypes = [P]
LIB.xmlXPathFreeContext.argtypes = [P]
LIB.xmlFreeDoc.argtypes = [P]


class XPathObject(ctypes.Structure):
    # Solo los dos primeros campos de xmlXPathObject: tipo y conjunto.
    _fields_ = [("type", ctypes.c_int), ("nodesetval", P)]


class NodeSet(ctypes.Structure):
    _fields_ = [("nodeNr", ctypes.c_int), ("nodeMax", ctypes.c_int), ("nodeTab", ctypes.POINTER(P))]


def node_set(obj):
    return ctypes.cast(obj, ctypes.POINTER(XPathObject)).contents.nodesetval


def run(case):
    data = case["xml"].encode("utf-8")
    doc = LIB.xmlReadMemory(data, len(data), None, b"UTF-8", XML_PARSE_NONET)
    if not doc:
        return {"error": "documento inválido"}
    ctx = LIB.xmlXPathNewContext(doc)
    objects = []
    try:
        for prefix, uri in case.get("namespaces", []):
            LIB.xmlXPathRegisterNs(ctx, prefix.encode(), uri.encode())
        context = doc
        if case.get("context"):
            found = LIB.xmlXPathNodeEval(doc, case["context"].encode(), ctx)
            objects.append(found)
            nodes = node_set(found) if found else None
            if not nodes:
                return {"error": "contexto no encontrado"}
            ns = ctypes.cast(nodes, ctypes.POINTER(NodeSet)).contents
            if ns.nodeNr != 1:
                return {"error": "contexto ambiguo"}
            context = ns.nodeTab[0]
        query = "(.//. | .//@* | .//namespace::*)[%s]" % case["expr"]
        result = LIB.xmlXPathNodeEval(context, query.encode(), ctx)
        if not result:
            return {"error": "xpath"}
        objects.append(result)
        prefixes = case.get("inclusive_prefixes") or []
        array = None
        if prefixes:
            array = (ctypes.c_char_p * (len(prefixes) + 1))(*[p.encode() for p in prefixes], None)
        out = ctypes.c_char_p()
        size = LIB.xmlC14NDocDumpMemory(
            doc,
            node_set(result),
            MODES[case["mode"]],
            array,
            1 if case.get("with_comments") else 0,
            ctypes.byref(out),
        )
        if size < 0:
            return {"error": "c14n"}
        return {"c14n": base64.b64encode(ctypes.string_at(out, size)).decode()}
    finally:
        for obj in objects:
            if obj:
                LIB.xmlXPathFreeObject(obj)
        LIB.xmlXPathFreeContext(ctx)
        LIB.xmlFreeDoc(doc)


def main():
    cases = json.load(sys.stdin)
    json.dump([run(case) for case in cases], sys.stdout)


if __name__ == "__main__":
    main()
