#!/usr/bin/env python3
"""Genera los casos sintéticos de paridad de greenter/report v5.3.0.

Los casos son entradas neutrales (sin resultados esperados). El oráculo PHP
(oracle/php/report/) y las pruebas Rust (crates/report/tests/) los ejecutan
por separado. Todos los datos son ficticios: RUC, documentos, logos y XML se
generan aquí y no proceden de contribuyentes ni de tests/Resources upstream.

Familias:
- html: render con HtmlReport (siete plantillas, plantillas de usuario,
  opciones de Twig, parámetros, errores);
- qr: contenidos para Encoder/Writer de Bacon (modo, versión, máscara, matriz, SVG);
- xml: entradas de XmlUtils (cadena, archivo y DOMDocument);
- resolver: clases para DefaultTemplateResolver.

Uso: python3 oracle/cases/report/generate_cases.py <directorio_salida>
"""

from __future__ import annotations

import base64
import copy
import json
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
import generate_cases as core  # noqa: E402

dec, dt, obj = core.dec, core.dt, core.obj


def raw(data: bytes) -> dict:
    return {"$bytes": base64.b64encode(data).decode("ascii")}


# --------------------------------------------------------------------------
# Imágenes sintéticas
# --------------------------------------------------------------------------

def png_1x1() -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
    ihdr = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)
    idat = zlib.compress(b"\x00\x2e\x7d\x32", 9)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")


GIF = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff!\xf9\x04\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;"
JPEG = b"\xff\xd8\xff\xe0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xff\xd9"
PNG = png_1x1()
SVG_LOGO = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#0a0"/></svg>'

FOOTER = '<p style="font-size:10px">Consulte su comprobante en <a href="https://example.test/cpe">example.test/cpe</a> &amp; gracias.</p>'


def params(**over) -> dict:
    """Parámetros de los tests upstream (getParamters), con datos sintéticos."""
    system = {"logo": raw(PNG), "hash": "xkhakjjuui293/=33w"}
    user = {
        "numIGV": 18,
        "header": "Telf: <b>(01) 000-0000</b>",
        "extras": [
            {"name": "CONDICION DE PAGO", "value": "Efectivo"},
            {"name": "vendedor ñandú", "value": "VENDEDOR <SINTÉTICO> & \"CÍA\""},
        ],
        "footer": FOOTER,
    }
    system.update(over.pop("system", {}))
    user.update(over.pop("user", {}))
    for key in [k for k, v in system.items() if v is DROP]:
        del system[key]
    for key in [k for k, v in user.items() if v is DROP]:
        del user[key]
    return {"system": system, "user": user}


DROP = object()


def default_params(**over) -> dict:
    """getDefaultParamters de los tests upstream."""
    out = params(**over)
    out["user"].pop("numIGV", None)
    out["user"].pop("extras", None)
    return out


STRICT = {"strict_variables": True, "globals": {"max_items": 7}}
DEFAULT: dict = {}


def html(case_id, description, template, document, parameters, options=None, template_dir=None):
    return {
        "id": case_id,
        "description": description,
        "template": template,
        "options": DEFAULT if options is None else options,
        "template_dir": template_dir,
        "document": document,
        "params": parameters,
    }


def set_fields(node: dict, **fields) -> dict:
    node = copy.deepcopy(node)
    node.update(fields)
    return node


# --------------------------------------------------------------------------
# Documentos
# --------------------------------------------------------------------------

def invoice(**over) -> dict:
    return set_fields(core.invoice_full(), **over)


def boleta(**over) -> dict:
    node = core.invoice_full()
    node.update(
        tipoDoc="03",
        serie="B001",
        correlativo="88",
        client=core.client(tipoDoc="1", numDoc="00000001", rznSocial="PERSONA SINTÉTICA"),
        detraccion=None,
        perception=None,
        totalAnticipos=None,
        anticipos=None,
        cuotas=None,
        guias=None,
    )
    node.update(over)
    return node


def minimal_invoice(**over) -> dict:
    node = obj(
        "Invoice",
        tipoDoc="01",
        serie="F001",
        correlativo="1",
        fechaEmision=dt("2024-03-15T00:00:00-05:00"),
        tipoMoneda="PEN",
        company=obj("Company", ruc="20000000001", razonSocial="EMISOR MINIMO"),
        client=obj("Client", tipoDoc="6", numDoc="20000000002", rznSocial="CLIENTE MINIMO"),
        mtoIGV=dec("0"),
        mtoImpVenta=dec("0"),
    )
    node.update(over)
    return node


def many_details(count: int) -> list:
    return [core.sale_detail(f"P{index:03d}", cantidad=dec(str(index)), mtoValorUnitario=dec(f"{index}.5")) for index in range(1, count + 1)]


def note(tipo: str = "07", **over) -> dict:
    return set_fields(core.note_full(tipo, "FC01" if tipo == "07" else "FD01", "01"), **over)


def despatch(**over) -> dict:
    return set_fields(core.despatch_full(), **over)


def summary(**over) -> dict:
    node = obj(
        "Summary",
        correlativo="001",
        fecGeneracion=dt("2024-01-31T20:00:00-05:00"),
        fecResumen=dt("2024-01-31T23:30:00-05:00"),
        moneda="PEN",
        company=core.company(),
        details=[
            core.summary_detail(),
            obj("SummaryDetail", tipoDoc="07", serieNro="BC01-2", estado="3", total=dec("50.00"), mtoIGV=dec("7.63"),
                mtoOperGravadas=dec("42.37"), mtoISC=dec("1.10"), mtoOtrosTributos=dec("0.40"), mtoOtrosCargos=dec("0.25"),
                mtoOperInafectas=dec("3"), mtoOperExoneradas=dec("4"), mtoOperGratuitas=dec("5"),
                docReferencia=obj("Document", tipoDoc="03", nroDoc="B001-1")),
            obj("SummaryDetail", tipoDoc="03", serieNro="B001-3", estado="2", total=dec("0"), mtoIGV=dec("0")),
        ],
    )
    node.update(over)
    return node


def voided(cls: str = "Voided", **over) -> dict:
    node = obj(
        cls,
        correlativo="00001",
        fecGeneracion=dt("2024-01-30T10:00:00-05:00"),
        fecComunicacion=dt("2024-01-31T23:30:00-05:00"),
        company=core.company(),
        details=[
            obj("VoidedDetail", tipoDoc="01", serie="F001", correlativo="123", desMotivoBaja="ERROR EN RUC"),
            obj("VoidedDetail", tipoDoc="07", serie="FC01", correlativo="4", desMotivoBaja="Anulación <con> & \"comillas\""),
        ],
    )
    node.update(over)
    return node


# --------------------------------------------------------------------------
# Casos HTML
# --------------------------------------------------------------------------

def html_cases() -> list[dict]:
    p, dp = params(), default_params()
    cases = [
        # Plantilla inicial y las siete plantillas.
        html("invoice-full", "Factura completa con todos los bloques (anticipo, cuotas, detracción, percepción)", "invoice.html.twig", invoice(), p),
        html("invoice-full-strict", "Configuración de los tests upstream: strict_variables y max_items=7", "invoice.html.twig", invoice(), p, STRICT),
        html("invoice-default-template", "Sin setTemplate: la plantilla inicial es invoice.html.twig", None, invoice(), p),
        html("boleta", "Boleta con cliente DNI, sin detracción ni anticipos", "invoice.html.twig", boleta(), p),
        html("boleta-strict", "Boleta en modo estricto", "invoice.html.twig", boleta(), p, STRICT),
        html("invoice-minimal", "Factura mínima: sin dirección, leyendas, detalles ni cuotas", "invoice.html.twig", minimal_invoice(), p),
        html("invoice-minimal-strict", "Factura mínima en modo estricto: cp.address nulo falla", "invoice.html.twig", minimal_invoice(), p, STRICT),
        html("invoice-one-detail", "Una sola línea", "invoice.html.twig", invoice(details=many_details(1)), p),
        html("invoice-many-details", "Ocho líneas con max_items=7: salto de página", "invoice.html.twig", invoice(details=many_details(8)), p, STRICT),
        html("invoice-max-items-equal", "Siete líneas con max_items=7: sin salto", "invoice.html.twig", invoice(details=many_details(7)), p, STRICT),
        html("invoice-empty-details", "Lista de detalles vacía", "invoice.html.twig", invoice(details=[]), p),
        html("invoice2-full", "Segunda factura (invoice2.html.twig)", "invoice2.html.twig", invoice(), p),
        html("invoice2-strict", "Segunda factura en modo estricto", "invoice2.html.twig", invoice(), p, STRICT),
        html("invoice2-null-serie", "invoice2 con serie y correlativo nulos: concatenación de null", "invoice2.html.twig", invoice(serie=None, correlativo=None), p),
        html("invoice2-raw-amounts", "invoice2 imprime importes float sin formato", "invoice2.html.twig",
             invoice(details=[core.sale_detail("X1", mtoValorUnitario=dec("18.0"), mtoValorVenta=dec("0.1")),
                              core.sale_detail("X2", mtoValorUnitario=dec("100000000000000"), mtoValorVenta=dec("0.00001")),
                              core.sale_detail("X3", mtoValorUnitario=dec("-2.50"), mtoValorVenta=dec("1234567.891"))]), p),
        html("note-credit", "Nota de crédito con la plantilla de factura", "invoice.html.twig", note("07"), p),
        html("note-debit-strict", "Nota de débito en modo estricto (sin getters de Invoice)", "invoice.html.twig", note("08"), p, STRICT),
        html("despatch-full", "Guía de remisión completa", "despatch.html.twig", despatch(), dp),
        html("despatch-strict-no-footer", "Test upstream: modo estricto sin footer", "despatch.html.twig", despatch(),
             default_params(user={"footer": DROP}), STRICT),
        html("despatch-qr-url", "Guía con params.system.qr (qrUrl)", "despatch.html.twig", despatch(),
             default_params(system={"qr": "https://e-factura.sunat.gob.pe/v1/contribuyente/gre/comprobantes/descargaqr?hashqr=SINTETICO123"})),
        html("despatch-no-transport", "Guía sin transportista, vehículo, choferes, bultos ni observación", "despatch.html.twig",
             despatch(envio=set_fields(core.shipment(), transportista=None, vehiculo=None, choferes=None, numBultos=0), observacion="0"), dp),
        html("despatch-one-driver", "Un chofer y vehículo sin secundarios", "despatch.html.twig",
             despatch(envio=set_fields(core.shipment(), vehiculo=core.vehicle("XYZ-999"),
                                       choferes=[obj("Driver", tipo="Principal", tipoDoc="1", nroDoc="00000009", nombres="ÚNICO", apellidos="CHOFER")])), dp),
        html("despatch-null-envio", "Guía sin envío: date(null) imprime la fecha actual", "despatch.html.twig", despatch(envio=None), dp),
        html("perception-full", "Percepción completa", "perception.html.twig", core.perception_full(), dp),
        html("perception-strict", "Percepción en modo estricto", "perception.html.twig", core.perception_full(), dp, STRICT),
        html("retention-full", "Retención completa", "retention.html.twig", core.retention_full(), dp),
        html("retention-minimal", "Retención sin detalles ni observación", "retention.html.twig",
             core.retention_full(details=None, observacion=None, tasa=dec("0")), dp),
        html("summary-full", "Resumen diario con referencia, percepción y tributos", "summary.html.twig", summary(), dp),
        html("summary-strict", "Resumen diario en modo estricto", "summary.html.twig", summary(), dp, STRICT),
        html("voided-full", "Comunicación de baja (RA)", "voided.html.twig", voided(), dp),
        html("reversion-full", "Resumen de reversiones (RR)", "voided.html.twig", voided("Reversion"), dp),
        html("voided-strict", "Baja en modo estricto", "voided.html.twig", voided(), dp, STRICT),
        html("voided-no-company", "Baja sin emisor: getName() falla", "voided.html.twig", voided(company=None), dp),
        # Semántica de PHP en las plantillas integradas.
        html("invoice-loose-tipo", "tipoDoc '1' es igual a '01' en PHP (in laxo)", "invoice.html.twig", invoice(tipoDoc="1"), p),
        html("invoice-loose-legend", "Leyenda '1000.0' == '1000' en PHP; duplicado: gana la primera", "invoice.html.twig",
             invoice(legends=[obj("Legend", code="1000.0", value="PRIMERA"), obj("Legend", code="1000", value="SEGUNDA"),
                              obj("Legend", code="2006", value="OPERACIÓN SUJETA A DETRACCIÓN")]), p),
        html("invoice-no-legends", "Sin leyendas: legend() devuelve ''", "invoice.html.twig", invoice(legends=None), p),
        html("invoice-string-zero", "compra '0' y hash '0' son falsos en PHP", "invoice.html.twig", invoice(compra="0"),
             params(system={"hash": "0"})),
        html("invoice-hash-empty", "hash vacío no se imprime", "invoice.html.twig", invoice(), params(system={"hash": ""})),
        html("invoice-no-hash", "Sin hash ni footer ni extras ni numIGV", "invoice.html.twig", invoice(),
             params(system={"hash": DROP}, user={"footer": DROP, "extras": DROP, "numIGV": DROP})),
        html("invoice-escaping", "Escape HTML de datos del documento; header/footer con raw", "invoice.html.twig",
             invoice(client=core.client(rznSocial="ÑANDÚ & CÍA \"LOS <ÁLAMOS>\" 'S.R.L.'"),
                     details=[core.sale_detail("<b>X</b>", descripcion="<script>alert('x')</script> & más")],
                     company=core.company(razonSocial="EMISOR & HIJOS <SAC>")),
             params(user={"header": "<script>/* confiable */</script><b>ok</b>", "footer": "<i>pie & fin</i>"})),
        html("invoice-amounts-edge", "Redondeo n_format: mitades, negativos, cero y más de 15 cifras", "invoice.html.twig",
             invoice(mtoOperGravadas=dec("1.005"), mtoIGV=dec("2.675"), mtoImpVenta=dec("1234567890123.455"),
                     redondeo=dec("-0.004"), icbper=dec("0.285"), mtoISC=dec("-1.5"), mtoOtrosTributos=dec("12345678901234567.89"),
                     sumOtrosCargos=dec("0.0000001"), mtoOperExportacion=dec("999999999999.995"),
                     details=[core.sale_detail("Z", cantidad=dec("0.125"), mtoValorUnitario=dec("-0.005"), mtoValorVenta=dec("1e3"))]), p),
        html("invoice-dates-utc", "Fecha UTC que en Lima es el día anterior y con hora", "invoice.html.twig",
             invoice(fechaEmision=dt("2024-02-01T03:00:00+00:00", "UTC"), fecVencimiento=dt("2024-03-01T04:59:59+00:00", "UTC")), p),
        html("invoice-midnight", "Medianoche en Lima: no imprime la hora", "invoice.html.twig",
             invoice(fechaEmision=dt("2024-02-01T05:00:00+00:00", "UTC"), fecVencimiento=None), p),
        html("invoice-usd-catalogs", "Moneda USD, cliente RUC, detracción con medio de pago con comillas", "invoice.html.twig",
             invoice(tipoMoneda="USD", detraccion=obj("Detraction", percent=dec("10"), mount=dec("23.76"), ctaBanco="0000",
                                                        codMedioPago="007", codBienDetraccion="017")), p),
        html("invoice-unknown-codes", "Códigos fuera de catálogo: cadena vacía", "invoice.html.twig",
             invoice(tipoMoneda="GBP", client=core.client(tipoDoc="4"),
                     detraccion=obj("Detraction", percent=dec("4"), mount=dec("1"), codMedioPago="998", codBienDetraccion="999")), p),
        html("invoice-gif-logo", "Logo GIF", "invoice.html.twig", invoice(), params(system={"logo": raw(GIF)})),
        html("invoice-jpeg-logo", "Logo JPEG (autodetección por defecto)", "invoice.html.twig", invoice(), params(system={"logo": raw(JPEG)})),
        html("invoice-no-autoescape", "Opción autoescape=false", "invoice.html.twig",
             invoice(client=core.client(rznSocial="A & B <C>")), p, {"autoescape": False}),
        # Errores.
        html("invoice-no-logo", "Sin logo: image_b64(null) lanza TypeError (substr)", "invoice.html.twig", invoice(),
             params(system={"logo": DROP})),
        html("invoice-no-company", "Sin emisor: qrCode() falla en getRuc() sobre null", "invoice.html.twig", invoice(company=None), p),
        html("invoice-no-date", "Sin fecha de emisión: date(null) usa ahora y qrCode() falla", "invoice.html.twig",
             invoice(fechaEmision=None), p),
        html("invoice-no-client", "Sin cliente: qrCode() falla", "invoice.html.twig", invoice(client=None), p),
        html("invoice-null-igv", "mtoIGV nulo: n_format(null) lanza TypeError", "invoice.html.twig", invoice(mtoIGV=None), p),
        html("invoice-strict-no-user", "Modo estricto sin params.user: Key 'user' no existe", "invoice.html.twig", invoice(),
             {"system": {"logo": raw(PNG)}}, STRICT),
        html("despatch-as-invoice", "Guía con la plantilla inicial: familia incorrecta", "invoice.html.twig", despatch(), p),
        html("summary-as-invoice-strict", "Resumen con la plantilla de factura en modo estricto", "invoice.html.twig", summary(), p, STRICT),
        html("missing-template", "Plantilla inexistente", "nothing.html.twig", invoice(), p),
        html("traversal-template", "Nombre con .. fuera de las carpetas", "../secret.html.twig", invoice(), p),
        html("traversal-nested", "Nombre que sube por encima de la raíz", "assets/../../invoice.html.twig", invoice(), p),
        html("dot-segments", "Nombre con ./ y // que sigue dentro", ".//./invoice.html.twig", invoice(), p),
        # Plantillas del usuario.
        html("user-override-invoice", "La carpeta del usuario tiene prioridad sobre invoice.html.twig", "invoice.html.twig", invoice(), p,
             template_dir="user-templates/override"),
        html("user-override-css", "La carpeta del usuario sustituye assets/style.css incluido por la plantilla integrada",
             "voided.html.twig", voided(), dp, template_dir="user-templates/css"),
        html("user-builtin-fallback", "Plantilla integrada cuando el usuario no la tiene", "summary.html.twig", summary(), dp,
             template_dir="user-templates/override"),
        html("user-syntax-error", "Plantilla del usuario con error de sintaxis", "invoice.html.twig", invoice(), p,
             template_dir="user-templates/bad"),
        html("user-undefined-function", "Plantilla del usuario que llama a una función inexistente", "unknown.html.twig", invoice(), p,
             template_dir="user-templates/bad"),
        # Filtros y funciones a través de plantillas comunes a Twig y MiniJinja.
        html("filter-n-format", "n_format con 0, 2, 3 y -1 decimales", "n_format.html.twig", minimal_invoice(),
             params(user={"nums": NUMS}), template_dir="user-templates/snippets"),
        html("filter-n-format-null", "n_format(null) lanza TypeError", "n_format_one.html.twig", minimal_invoice(),
             params(user={"one": None}), template_dir="user-templates/snippets"),
        html("filter-n-format-string", "n_format('18') lanza TypeError (strict_types)", "n_format_one.html.twig", minimal_invoice(),
             params(user={"one": "18"}), template_dir="user-templates/snippets"),
        html("filter-image", "image_b64: PNG, GIF, JPEG, MIME explícito, vacío y '0'", "image.html.twig", minimal_invoice(),
             params(user={"gif": raw(GIF), "jpeg": raw(JPEG), "svg": SVG_LOGO, "empty": ""}), template_dir="user-templates/snippets"),
        html("filter-catalog-types", "catalog con int, float, bool y null como clave", "catalog.html.twig", minimal_invoice(),
             params(user={"keys": [1, dec("6.9"), True, None, "1", "01", "PEN", "", "x"]}), template_dir="user-templates/snippets"),
        html("filter-upper-length", "upper con acentos, length de null, cadena, lista y mapa", "upper.html.twig", minimal_invoice(),
             params(user={"words": ["ñandú", "straße", "ǆ", "", None], "map": {"a": 1, "b": 2}}), template_dir="user-templates/snippets"),
        html("function-legend", "legend con códigos repetidos, ausentes y comparación laxa", "legend.html.twig",
             minimal_invoice(legends=[obj("Legend", code="1000", value="A"), obj("Legend", code="1000", value="B"),
                                      obj("Legend", code="02", value="C"), obj("Legend", code=None, value="NULO")]), p,
             template_dir="user-templates/snippets"),
        html("function-qr-sale", "qrCode de factura con importes nulos y cliente sin documento", "qr_sale.html.twig",
             minimal_invoice(mtoIGV=None, mtoImpVenta=dec("1234.5"), client=obj("Client", rznSocial="SIN DOC")), p,
             template_dir="user-templates/snippets"),
        html("function-qr-note", "qrCode de nota de crédito", "qr_sale.html.twig", note("07"), p, template_dir="user-templates/snippets"),
        html("function-qr-despatch", "qrCodeDespatch de una guía", "qr_despatch.html.twig", despatch(), p,
             template_dir="user-templates/snippets"),
        html("function-qr-despatch-utc", "qrCodeDespatch usa la zona propia de la fecha (sin convertir a Lima)", "qr_despatch.html.twig",
             despatch(fechaEmision=dt("2024-06-16T03:00:00+00:00", "UTC")), p, template_dir="user-templates/snippets"),
        html("function-qr-despatch-no-dest", "qrCodeDespatch sin destinatario falla", "qr_despatch.html.twig",
             despatch(destinatario=None), p, template_dir="user-templates/snippets"),
        html("despatch-qr-null", "params.system.qr presente con null: qrUrl(null) lanza TypeError", "despatch.html.twig", despatch(),
             default_params(system={"qr": None})),
        html("invoice-logo-over-limit", "Logo de 10 MiB + 1 byte: PHP lo incrusta; Rust aplica el límite por defecto", "invoice.html.twig",
             invoice(), params(system={"logo": {"$repeat": raw(PNG), "length": 10 * 1024 * 1024 + 1}})),
        html("user-symlink-escape", "Plantilla del usuario que es un enlace simbólico fuera de su carpeta", "invoice.html.twig",
             invoice(), p, template_dir="user-templates/symlink"),
        html("user-twig-only-syntax", "Plantilla del usuario con sintaxis exclusiva de Twig (?:, ??, |format)", "twig_only.html.twig",
             minimal_invoice(), p, template_dir="user-templates/bad"),
        html("user-unknown-function-dead-branch", "Función inexistente en una rama no ejecutada: Twig falla al compilar",
             "dead_branch.html.twig", minimal_invoice(), p, template_dir="user-templates/bad"),
        html("filter-builtins", "Filtros nativos de Twig: e, lower, default con '', null, [], false y 0", "builtins.html.twig",
             minimal_invoice(), params(user={"s": "A&B <Ñ> 'x' \"y\"", "empty": "", "none": None, "list": [], "no": False, "zero": 0}),
             template_dir="user-templates/snippets"),
        html("function-qr-sale-on-despatch", "qrCode con una guía: método inexistente", "qr_sale.html.twig", despatch(), p,
             template_dir="user-templates/snippets"),
    ]
    for key, url in QR_URLS:
        cases.append(html(f"function-qr-url-{key}", f"qrUrl: {key}", "qr_url.html.twig", minimal_invoice(),
                          params(system={"qr": url}), template_dir="user-templates/snippets"))
    for key, fmt, instant, tz in DATE_FORMATS:
        cases.append(html(f"filter-date-{key}", f"date('{fmt}') sobre {instant} {tz}", "date.html.twig",
                          minimal_invoice(fechaEmision=dt(instant, tz)), params(user={"fmt": fmt}),
                          template_dir="user-templates/snippets"))
    return cases


NUMS = [dec(x) for x in ["0", "-0.0", "1.005", "2.675", "0.285", "-0.004", "-1.5", "1.5", "0.5", "-0.5", "1234.5678",
                         "1e3", "0.0000001", "999999999999.995", "1234567890123.455", "12345678901234567.89",
                         "4503599627370496", "9007199254740993", "100000000000000000000", "0.045", "1.955", "8.325",
                         "3.14159", "-273.15"]] + [0, 7, -12, 1234567]

QR_URLS = [
    ("ascii", "https://example.test/cpe?id=20000000001-01-F001-1"),
    ("unicode", "https://example.test/búsqueda?q=ñandú€"),
    ("alnum", "HTTPS://EXAMPLE.TEST/ABC 123"),
    ("numeric", "20000000001"),
    ("one-char", "a"),
    ("v9-v10-byte", "x" * 152),
    ("v10-byte", "x" * 153),
    ("long", "https://example.test/" + "a" * 600),
    ("max-byte", "x" * 1660),
    ("too-big", "x" * 1664),
    ("empty", ""),
    ("int", 12345),
]

DATE_FORMATS = [
    ("all-lima", "d D j l N S w z W F m M n t L o X x Y y a A B g G h H i s u v e I O P p T Z U", "2024-02-29T13:05:09.123456-05:00", "America/Lima"),
    ("all-utc", "d D j l N S w z W F m M n t L o X x Y y a A B g G h H i s u v e I O P p T Z U", "2024-12-31T23:59:59+00:00", "UTC"),
    ("iso-rfc", "c|r", "2021-01-03T00:00:00+00:00", "UTC"),
    ("escaped", "\\Y\\m\\d Y-m-d \\\\ H\\h", "2024-03-15T10:20:30-05:00", "America/Lima"),
    ("dst-1994", "Y-m-d H:i:s T I O", "1994-02-15T04:30:00+00:00", "UTC"),
    ("lmt-1900", "Y-m-d H:i:s T O P", "1900-01-01T05:05:00+00:00", "UTC"),
    ("year-0999", "Y y o X x", "0999-06-15T12:00:00+00:00", "UTC"),
    ("ordinals", "jS", "2024-03-22T12:00:00-05:00", "America/Lima"),
    ("noon-midnight", "g G h H a A", "2024-03-22T00:30:00-05:00", "America/Lima"),
    ("default-format", "", "2024-03-22T12:00:00-05:00", "America/Lima"),
]


USER_TEMPLATES = {
    "override/invoice.html.twig": "<p>PROPIA {{ doc.serie }}-{{ doc.correlativo }} {{ doc.tipoDoc|catalog('01') }}</p>\n<p>{{ doc.mtoImpVenta|n_format }} {{ doc.client.rznSocial }}</p>\n",
    "css/assets/style.css": "body{color:#123456}",
    "bad/invoice.html.twig": "{% if %}roto{% endif %}\n",
    "bad/unknown.html.twig": "{{ inexistente(doc) }}\n",
    "bad/twig_only.html.twig": "{{ doc.serie ? 'S' : 'N' }}|{{ doc.nada ?? 'defecto' }}|{{ '%s-%s'|format(doc.serie, doc.correlativo) }}\n",
    "bad/dead_branch.html.twig": "{% if false %}{{ inexistente() }}{% endif %}ok\n",
    "outside.html.twig": "FUERA DE LA CARPETA {{ doc.serie }}\n",
    "snippets/builtins.html.twig": "{{ params.user.s|e }}|{{ params.user.s|lower }}|{{ params.user.empty|default('D1') }}|{{ params.user.none|default('D2') }}|{{ params.user.list|default('D3') }}|{{ params.user.no|default('D4') }}|{{ params.user.zero|default('D5') }}|{{ params.user.missing|default('D6') }}\n",
    "snippets/n_format.html.twig": "{% for n in params.user.nums %}{{ n|n_format }}|{{ n|n_format(0) }}|{{ n|n_format(3) }}|{{ n|n_format(-1) }}\n{% endfor %}",
    "snippets/n_format_one.html.twig": "[{{ params.user.one|n_format }}]\n",
    "snippets/image.html.twig": "{{ params.system.logo|image_b64 }}\n{{ params.user.gif|image_b64 }}\n{{ params.user.jpeg|image_b64 }}\n{{ params.user.svg|image_b64('svg+xml') }}\n{{ params.system.logo|image_b64('webp') }}\n{{ params.user.gif|image_b64('0') }}\n{{ params.user.empty|image_b64 }}\n",
    "snippets/catalog.html.twig": "{% for k in params.user.keys %}[{{ k|catalog('06') }}|{{ k|catalog('01') }}|{{ k|catalog('02') }}|{{ 'PEN'|catalog(k) }}]\n{% endfor %}",
    "snippets/upper.html.twig": "{% for w in params.user.words %}[{{ w|upper }}|{{ w|length }}]{% endfor %}\n{{ params.user.map|length }}|{{ params.user.words|length }}|{{ doc.legends|length }}\n",
    "snippets/legend.html.twig": "[{{ legend(doc.legends, '1000') }}][{{ legend(doc.legends, '2') }}][{{ legend(doc.legends, '9999') }}][{{ legend(doc.legends, '') }}][{{ legend(doc.guias, '1000') }}]\n",
    "snippets/qr_sale.html.twig": "{{ qrCode(doc)|raw }}",
    "snippets/qr_despatch.html.twig": "{{ qrCodeDespatch(doc)|raw }}",
    "snippets/qr_url.html.twig": "{{ qrUrl(params.system.qr)|raw }}",
    "snippets/date.html.twig": "{% if params.user.fmt %}{{ doc.fechaEmision|date(params.user.fmt) }}{% else %}{{ doc.fechaEmision|date }}{% endif %}\n",
}


# --------------------------------------------------------------------------
# QR, XmlUtils y resolver
# --------------------------------------------------------------------------

def qr_cases() -> list[dict]:
    texts = [
        ("sale", "20000000001|01|F001|123|36.00|237.59|2024-01-31|6|20000000002|"),
        ("boleta", "20000000001|03|B001|1|0.00|10.00|2024-03-15|1|00000001|"),
        ("despatch", "20000000001|09|T001|12|2024-06-15|6|20000000002|"),
        ("url", "https://e-factura.sunat.gob.pe/v1/contribuyente/gre/comprobantes/descargaqr?hashqr=SINTETICO123"),
        ("alnum", "HTTPS://EXAMPLE.TEST/ABC"),
        ("numeric", "1234567890"),
        ("zero", "0"),
        ("one-byte", "a"),
        ("unicode", "ñandú € 中文"),
        ("accented", "Dirección: Av. Perú 123 – ÁÉÍÓÚ"),
    ]
    for n in (17, 18, 32, 33, 152, 153, 154, 230, 231, 400, 1000, 1660, 1661, 1662, 1663):
        texts.append((f"byte-{n}", "x" * n))
    for n in (27, 28, 400, 3993, 3994):
        texts.append((f"numeric-{n}", "7" * n))
    for n in (16, 17, 400, 2420, 2421):
        texts.append((f"alnum-{n}", "A" * n))
    texts.append(("empty", ""))
    cases = [{"id": f"qr-{key}", "content": raw(text.encode("utf-8"))} for key, text in texts]
    cases.append({"id": "qr-invalid-utf8", "content": raw(b"\xff\xfe")})
    return cases


EXT = "urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2"
DS = "http://www.w3.org/2000/09/xmldsig#"


def signature(digest: str, refs: int = 1, prefix_ds: str = "ds") -> str:
    references = "".join(
        f"<{prefix_ds}:Reference URI=\"#r{i}\"><{prefix_ds}:DigestValue>{digest}{'' if i == 0 else i}</{prefix_ds}:DigestValue></{prefix_ds}:Reference>"
        for i in range(refs)
    )
    return (f"<{prefix_ds}:Signature xmlns:{prefix_ds}=\"{DS}\" Id=\"SignSintetica\"><{prefix_ds}:SignedInfo>{references}"
            f"</{prefix_ds}:SignedInfo><{prefix_ds}:SignatureValue>AAAA</{prefix_ds}:SignatureValue></{prefix_ds}:Signature>")


def ubl(*extensions: str, root: str = "Invoice", ext_prefix: str = "ext", decl: str = '<?xml version="1.0" encoding="UTF-8"?>\n') -> str:
    exts = "".join(f"<{ext_prefix}:UBLExtension><{ext_prefix}:ExtensionContent>{content}</{ext_prefix}:ExtensionContent></{ext_prefix}:UBLExtension>"
                   for content in extensions)
    return (f"{decl}<{root} xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:{root}-2\" xmlns:{ext_prefix}=\"{EXT}\">"
            f"<{ext_prefix}:UBLExtensions>{exts}</{ext_prefix}:UBLExtensions><cbc:ID xmlns:cbc=\"urn:x\">F001-1</cbc:ID></{root}>")


def xml_cases() -> list[dict]:
    items = [
        ("signed", ubl(signature("SINTETICO+Digest/1="))),
        ("two-extensions", ubl("<x>sin firma</x>", signature("ULTIMA="))),
        ("last-without-signature", ubl(signature("PRIMERA="), "<x/>")),
        ("both-signed", ubl(signature("PRIMERA="), signature("SEGUNDA="))),
        ("multiple-references", ubl(signature("REF", refs=3))),
        ("other-prefixes", ubl(signature("PREFIJOS=", prefix_ds="sig"), ext_prefix="e")),
        ("wrong-namespace", ubl(signature("X=").replace(DS, "http://example.test/ds"))),
        ("no-extensions", "<Invoice><cbc:ID xmlns:cbc=\"urn:x\">1</cbc:ID></Invoice>"),
        ("empty-extensions", ubl()),
        ("html", "<h1>EMPTY</h1>"),
        ("empty-digest", ubl(signature(""))),
        ("whitespace-digest", ubl(signature("\n  ESPACIOS=\n  "))),
        ("cdata-digest", ubl(signature("<![CDATA[CDATA=]]>"))),
        ("nested-digest", ubl(signature("A<b>B</b>C"))),
        ("summary-root", ubl(signature("RESUMEN="), root="SummaryDocuments")),
        ("latin1", ubl(signature("LATIN1="), decl='<?xml version="1.0" encoding="ISO-8859-1"?>\n').replace("F001-1", "Peñón")),
        ("malformed", "<Invoice><ext:UBLExtensions>"),
        ("empty", ""),
        ("dtd-entity", '<!DOCTYPE Invoice [<!ENTITY h "ENTIDAD=">]>' + ubl(signature("&h;"), decl="")),
        ("dtd-external", '<!DOCTYPE Invoice SYSTEM "file:///etc/passwd">' + ubl(signature("EXTERNA="), decl="")),
        ("deep", "<a>" * 300 + "</a>" * 300),
    ]
    cases = []
    for key, text in items:
        data = text.encode("iso-8859-1" if key == "latin1" else "utf-8")
        cases.append({"id": f"xml-{key}", "xml": raw(data)})
    cases.append({"id": "xml-missing-file", "file": "no-existe.xml"})
    return cases


def resolver_cases() -> list[dict]:
    classes = ["Invoice", "Note", "Retention", "Perception", "Despatch", "Summary", "Voided", "Reversion", "BaseSale", "Receipt"]
    return [{"id": f"resolver-{c.lower()}", "class": c} for c in classes]


def main() -> None:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "tests/reference/report")
    cases_dir = out / "cases"
    cases_dir.mkdir(parents=True, exist_ok=True)
    for name, cases in (("html", html_cases()), ("qr", qr_cases()), ("xml", xml_cases()), ("resolver", resolver_cases())):
        ids = [c["id"] for c in cases]
        if len(ids) != len(set(ids)):
            raise SystemExit(f"ids duplicados en {name}")
        (cases_dir / f"{name}.json").write_text(
            json.dumps({"family": name, "cases": cases}, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    for rel, text in USER_TEMPLATES.items():
        path = cases_dir / "user-templates" / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    # Enlace simbólico relativo que apunta fuera de la carpeta de plantillas.
    link = cases_dir / "user-templates" / "symlink" / "invoice.html.twig"
    link.parent.mkdir(parents=True, exist_ok=True)
    if link.is_symlink() or link.exists():
        link.unlink()
    link.symlink_to("../outside.html.twig")


if __name__ == "__main__":
    main()
