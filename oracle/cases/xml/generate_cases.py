#!/usr/bin/env python3
"""Genera los casos sintéticos de paridad de greenter/xml.

Cada caso indica el builder PHP, las opciones de Twig y el modelo de entrada
(misma gramática que los casos de core). Los datos son ficticios.

Uso: python3 oracle/cases/xml/generate_cases.py [directorio_salida]
"""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
import generate_cases as core  # noqa: E402

dec, dt, obj = core.dec, core.dt, core.obj

# Opciones de Twig observadas en el código y pruebas de Greenter.
DEFAULT = {}  # TwigBuilder sin opciones: autoescape "html", strict_variables false.
CPEFORGE = {"autoescape": False}  # Fachadas See y Api (cpeforge y la referencia).
TESTS = {"cache": False, "strict_variables": True, "autoescape": False}  # pruebas PHP del paquete.
STRICT_HTML = {"strict_variables": True}

OPTION_SETS = {"default": DEFAULT, "cpeforge": CPEFORGE, "tests": TESTS}


def case(case_id, description, builder, node, options=None):
    return {
        "id": case_id,
        "description": description,
        "builder": builder,
        "options": DEFAULT if options is None else options,
        "input": node,
    }


def with_options(base_id, description, builder, node, sets=("default", "cpeforge", "tests")):
    return [case(f"{base_id}-{name}", f"{description} [{name}]", builder, copy.deepcopy(node), OPTION_SETS[name]) for name in sets]


def set_fields(node, **fields):
    node = copy.deepcopy(node)
    node.update(fields)
    return node


# --------------------------------------------------------------------------
# Factura y boleta
# --------------------------------------------------------------------------

def invoice(version="2.1", **over):
    node = core.invoice_full()
    node["ublVersion"] = version
    node.update(over)
    return node


def minimal_invoice(version="2.1", **over):
    node = obj(
        "Invoice",
        ublVersion=version,
        tipoOperacion="0101",
        tipoDoc="01",
        serie="F001",
        correlativo="1",
        fechaEmision=dt("2024-03-15T10:20:30-05:00"),
        tipoMoneda="PEN",
        company=core.company(),
        client=obj("Client", tipoDoc="6", numDoc="20000000002", rznSocial="CLIENTE"),
        mtoOperGravadas=dec("100"),
        mtoIGV=dec("18"),
        totalImpuestos=dec("18"),
        valorVenta=dec("100"),
        subTotal=dec("118"),
        mtoImpVenta=dec("118"),
        details=[
            obj(
                "SaleDetail",
                codProducto="P1",
                unidad="NIU",
                cantidad=dec("1"),
                descripcion="ITEM",
                mtoBaseIgv=dec("100"),
                porcentajeIgv=dec("18"),
                igv=dec("18"),
                tipAfeIgv="10",
                totalImpuestos=dec("18"),
                mtoValorVenta=dec("100"),
                mtoValorUnitario=dec("100"),
                mtoPrecioUnitario=dec("118"),
            )
        ],
    )
    node.update(over)
    return node


def line(code, afect, **over):
    node = obj(
        "SaleDetail",
        codProducto=code,
        unidad="NIU",
        cantidad=dec("2"),
        descripcion="ITEM " + code,
        mtoBaseIgv=dec("100"),
        porcentajeIgv=dec("18"),
        igv=dec("18"),
        tipAfeIgv=afect,
        totalImpuestos=dec("18"),
        mtoValorVenta=dec("100"),
        mtoValorUnitario=dec("50"),
        mtoPrecioUnitario=dec("59"),
    )
    node.update(over)
    return node


SPECIAL = "A & B <S.A.> \"comillas\" 'apostrofe'"


def invoice_cases():
    b = "InvoiceBuilder"
    cases = []
    cases += with_options("inv21-full", "Factura UBL 2.1 completa", b, invoice("2.1"))
    cases += with_options("inv20-full", "Factura UBL 2.0 completa con guía embebida, detracción y percepción", b, invoice("2.0"))
    cases += with_options("inv21-minimal", "Factura UBL 2.1 mínima", b, minimal_invoice("2.1"))
    cases += with_options("inv20-minimal", "Factura UBL 2.0 mínima", b, minimal_invoice("2.0"))
    boleta = minimal_invoice("2.1", tipoDoc="03", serie="B001", client=obj("Client", tipoDoc="1", numDoc="00000001", rznSocial="CLIENTE DNI"))
    cases += with_options("boleta21", "Boleta UBL 2.1", b, boleta, ("default", "tests"))
    cases += with_options("boleta20", "Boleta UBL 2.0", b, set_fields(boleta, ublVersion="2.0"), ("default", "tests"))

    # Tributos por afectación y totales opcionales.
    taxes = minimal_invoice(
        "2.1",
        mtoOperInafectas=dec("10"),
        mtoOperExoneradas=dec("20"),
        mtoOperExportacion=dec("30"),
        mtoOperGratuitas=dec("40"),
        mtoIGVGratuitas=dec("7.2"),
        mtoBaseIsc=dec("100"),
        mtoISC=dec("17"),
        mtoBaseIvap=dec("50"),
        mtoIvap=dec("2"),
        mtoBaseOth=dec("60"),
        mtoOtrosTributos=dec("6"),
        icbper=dec("0.80"),
        sumOtrosDescuentos=dec("1"),
        sumOtrosCargos=dec("2"),
        totalAnticipos=dec("3"),
        redondeo=dec("0.01"),
        details=[
            line("A10", "10", isc=dec("17"), mtoBaseIsc=dec("100"), porcentajeIsc=dec("17"), tipSisIsc="01"),
            line("A17", "17"),
            line("A20", "20", igv=dec("0")),
            line("A30", "30", igv=dec("0")),
            line("A40", "40", igv=dec("0")),
            line("A11", "11", mtoValorGratuito=dec("50"), igv=dec("9")),
            line("A21", "21", mtoValorGratuito=dec("0.10")),
            line("A99", "99"),
            line("ANULL", None),
            line("AVACIO", ""),
            line("ATEXTO", "10abc"),
            line("AESP", " 20"),
            line("AOTH", "10", otroTributo=dec("6"), mtoBaseOth=dec("60"), porcentajeOth=dec("10")),
            line("ABOLSA", "10", icbper=dec("0.80"), factorIcbper=dec("0.40"), cantidad=dec("2")),
        ],
    )
    cases += with_options("inv21-taxes", "Todas las afectaciones y tributos en UBL 2.1", b, taxes, ("default", "tests"))
    cases += with_options("inv20-taxes", "Todas las afectaciones y tributos en UBL 2.0", b, set_fields(taxes, ublVersion="2.0", mtoDescuentos=dec("5")), ("default", "tests"))
    zero_totals = minimal_invoice(
        "2.1",
        mtoOperGravadas=dec("0"),
        mtoOperInafectas=dec("0"),
        mtoOperExoneradas=dec("0.00"),
        mtoOperGratuitas=dec("0"),
        mtoOperExportacion=dec("0"),
        mtoISC=dec("0"),
        mtoIvap=dec("0"),
        mtoOtrosTributos=dec("0"),
        icbper=dec("0"),
        sumOtrosDescuentos=dec("0"),
        sumOtrosCargos=dec("0"),
        totalAnticipos=dec("0"),
        redondeo=dec("0"),
        subTotal=None,
    )
    cases += with_options("inv21-zero-totals", "Totales en cero: `is not null` frente a valor verdadero", b, zero_totals, ("default",))
    cases += with_options("inv20-zero-totals", "Totales en cero en UBL 2.0", b, set_fields(zero_totals, ublVersion="2.0"), ("default",))
    null_totals = minimal_invoice("2.1", mtoOperGravadas=None, mtoIGV=None, totalImpuestos=None, mtoImpVenta=None, valorVenta=None)
    cases += with_options("inv21-null-totals", "Importes nulos en n_format (PHP los formatea como 0.00)", b, null_totals, ("default", "tests"))
    cases += with_options("inv20-null-totals", "Importes nulos en UBL 2.0 (default(0))", b, set_fields(null_totals, ublVersion="2.0"), ("default", "tests"))

    # Formas de pago y cuotas.
    credit = minimal_invoice(
        "2.1",
        formaPago=obj("FormaPagoCredito", [dec("118")]),
        cuotas=[obj("Cuota", monto=dec("10"), fechaPago=dt("2024-04-01T00:00:00-05:00")) for _ in range(11)]
        + [obj("Cuota", moneda="USD", monto=dec("8.5"), fechaPago=dt("2024-05-01T03:00:00+00:00", "UTC"))],
    )
    cases += with_options("inv21-credit", "Crédito sin moneda (default a tipoMoneda) y 12 cuotas", b, credit, ("default", "tests"))
    cases += with_options("inv21-contado", "Pago al contado sin monto", b, minimal_invoice("2.1", formaPago=obj("FormaPagoContado")), ("default",))
    cases += with_options(
        "inv21-forma-pago-zero",
        "Forma de pago con monto 0 (falso en Twig) y moneda vacía",
        b,
        minimal_invoice("2.1", formaPago=obj("PaymentTerms", tipo="Credito", monto=dec("0"), moneda=""), cuotas=[]),
        ("default",),
    )
    cases += with_options(
        "inv21-cuota-empty-currency",
        "Cuota con moneda vacía usa la del documento",
        b,
        minimal_invoice("2.1", cuotas=[obj("Cuota", moneda="", monto=dec("118"), fechaPago=dt("2024-04-01T00:00:00-05:00"))]),
        ("default",),
    )

    # Cargos, descuentos y anticipos.
    charges = minimal_invoice(
        "2.1",
        cargos=[core.charge("50", "0.02", "4.00", "200.00"), obj("Charge", codTipo="51", monto=dec("1"), montoBase=dec("10"))],
        descuentos=[core.charge("02", "0.123456", "2.00", "200.00"), obj("Charge", codTipo="03", monto=dec("1.5"), montoBase=dec("10"))],
        anticipos=[obj("Prepayment", tipoDocRel="02", nroDocRel="F001-1", total=dec("10")), obj("Prepayment", tipoDocRel="03", nroDocRel="B001-2", total=dec("5.5"))],
        totalAnticipos=dec("15.5"),
        details=[line("C1", "10", cargos=[core.charge("47", "0.1", "10.50", "105.00")], descuentos=[core.charge("00", "0.05", "5.25", "105.00")])],
    )
    cases += with_options("inv21-charges", "Cargos y descuentos globales y por línea, anticipos", b, charges, ("default", "tests"))
    cases += with_options("inv20-charges", "Anticipos y descuento por línea en UBL 2.0", b, set_fields(charges, ublVersion="2.0", details=[line("C1", "10", descuento=dec("5.25"))]), ("default",))

    # Atributos de línea.
    attrs = minimal_invoice(
        "2.1",
        details=[
            line(
                "AT",
                "10",
                atributos=[
                    obj("DetailAttribute", code="5010", name="Placa", value="ABC-123"),
                    obj("DetailAttribute", code="5011", name="Cero", value="0"),
                    obj("DetailAttribute", code="5012", name="Vacio", value=""),
                    obj("DetailAttribute", code="5013", name="Nulo"),
                    obj("DetailAttribute", code="3050", name="Periodo", fecInicio=dt("2024-03-01T02:00:00+00:00", "UTC", "DateTime"), fecFin=dt("2024-03-31T12:00:00-05:00", cls="DateTime")),
                    obj("DetailAttribute", code="3051", name="Duracion", duracion=5),
                    obj("DetailAttribute", code="3052", name="DuracionCero", duracion=0),
                ],
            )
        ],
    )
    cases += with_options("inv21-attributes", "Atributos: valor 0, vacío, nulo, periodos y duración", b, attrs, ("default",))

    # Partes: comercial, contacto, vendedor, entrega.
    parties = minimal_invoice(
        "2.1",
        company=core.company(nombreComercial="0", email=None, telephone="01-555"),
        client=core.client(email="c@example.test", telephone=None, address=obj("Address", direccion="SIN UBIGEO")),
        seller=obj("Client", tipoDoc="1", numDoc="00000009", rznSocial="VENDEDOR"),
        direccionEntrega=obj("Address", ubigueo="150101", direccion="ENTREGA", provincia="LIMA", departamento="LIMA", distrito="LIMA"),
        compra="OC-9",
        observacion="Obs",
        guias=[],
        relDocs=[obj("Document", tipoDoc="99", nroDoc="R-1")],
    )
    cases += with_options("inv21-parties", "Nombre comercial \"0\", contactos parciales, vendedor y entrega", b, parties, ("default", "tests"))
    cases += with_options("inv20-parties", "Partes en UBL 2.0", b, set_fields(parties, ublVersion="2.0"), ("default", "tests"))
    cases += with_options(
        "inv21-no-contact",
        "Sin correo ni teléfono y cliente sin dirección",
        b,
        minimal_invoice("2.1", company=core.company(email=None, telephone=None), client=obj("Client", tipoDoc="6", numDoc="2", rznSocial="X")),
        ("default",),
    )

    # Texto especial, espacios y Unicode.
    special = minimal_invoice(
        "2.1",
        serie="F&1",
        correlativo="<1>",
        company=core.company(razonSocial=SPECIAL, nombreComercial="ÑANDÚ 😀  ", telephone="  "),
        client=obj("Client", tipoDoc="6", numDoc="\"2\"", rznSocial="x>  <y"),
        observacion="  espacios  ",
        details=[line("S1", "10", descripcion="Descripción ]]> fin", codProducto="P'1")],
    )
    cases += with_options("inv21-special", "Escape HTML, CDATA con ]]>, espacios entre etiquetas y Unicode", b, special)
    cases += with_options("inv20-special", "Texto especial en UBL 2.0", b, set_fields(special, ublVersion="2.0"), ("default", "cpeforge"))

    # Fechas.
    dates = minimal_invoice(
        "2.1",
        fechaEmision=dt("2024-03-01T04:30:15+00:00", "UTC"),
        fecVencimiento=dt("2024-12-31T23:59:59+09:00", "+09:00"),
    )
    cases += with_options("inv21-dates-utc", "Fechas en otras zonas se imprimen en hora de Lima", b, dates, ("default",))
    cases += with_options("inv20-dates-utc", "Fechas en otras zonas en UBL 2.0", b, set_fields(dates, ublVersion="2.0"), ("default",))
    cases += with_options(
        "inv21-dates-mutable",
        "DateTime mutable con desfase",
        b,
        minimal_invoice("2.1", fechaEmision=dt("2024-02-29T23:00:00-05:00", "America/Lima", "DateTime")),
        ("default",),
    )
    cases += with_options("inv21-null-date", "fechaEmision nula: la plantilla usa la hora actual", b, minimal_invoice("2.1", fechaEmision=None), ("default", "tests"))

    # Importes con formato.
    amounts = minimal_invoice(
        "2.1",
        mtoOperGravadas=dec("1234567.891"),
        mtoIGV=dec("-0.004"),
        totalImpuestos=dec("-0.005"),
        mtoImpVenta=dec("99999999999.995"),
        valorVenta=dec("1.005"),
        subTotal=dec("2.675"),
        redondeo=dec("-0.00"),
        details=[
            line("N1", "10", cantidad=dec("0.1"), mtoValorUnitario=dec("10.343434223423423"), mtoPrecioUnitario=dec("432.23"), porcentajeIgv=dec("18.00")),
            line("N2", "10", cantidad=dec("1e-6"), mtoValorUnitario=dec("0.000001"), mtoPrecioUnitario=dec("1e15"), porcentajeIgv=dec("0.18")),
            line("N3", "10", cantidad=dec("123456789012345"), mtoValorUnitario=dec("59.0"), mtoPrecioUnitario=dec("0.12345678901"), porcentajeIgv=dec("-0.0")),
            line("N4", "10", cantidad=dec("100000000000000"), mtoValorUnitario=dec("99999999999999"), mtoPrecioUnitario=dec("0.0001"), porcentajeIgv=dec("0.00001")),
        ],
        cargos=[core.charge("50", "0.123456", "1", "1"), core.charge("50", "1e-6", "1", "1"), core.charge("50", "0.12345", "1", "1")],
    )
    cases += with_options("inv21-amounts", "Formato n_format, n_format_limit e impresión de floats", b, amounts, ("default",))
    precision = minimal_invoice(
        "2.1",
        mtoImpVenta=dec("12345678901234567.89"),
        valorVenta=dec("1.0049999999999999"),
        details=[line("P1", "10", mtoValorUnitario=dec("123456789012345.67"), cantidad=dec("0.12345678901234567"))],
    )
    cases += with_options("inv21-precision", "Importes con más de 15 cifras: PHP ya perdió precisión al usar float", b, precision, ("default",))
    cases += with_options("inv20-amounts", "Formato de importes en UBL 2.0", b, set_fields(amounts, ublVersion="2.0"), ("default",))

    # Vacíos, nulos y estricto.
    cases += with_options("inv-empty", "Invoice vacío: ublVersion 2.0 por defecto y todo nulo", b, obj("Invoice"))
    cases += with_options("inv21-empty", "Invoice 2.1 vacío", b, obj("Invoice", ublVersion="2.1"))
    cases += with_options("inv21-no-client", "Sin cliente: vacío o error estricto", b, minimal_invoice("2.1", client=None))
    cases += with_options("inv21-company-no-address", "Emisor sin dirección", b, minimal_invoice("2.1", company=core.company(address=None)))
    cases += with_options("inv20-company-no-address", "Emisor sin dirección en UBL 2.0", b, minimal_invoice("2.0", company=core.company(address=None)))
    cases += with_options(
        "inv20-embedded-partial",
        "Guía embebida sin llegada ni transportista",
        b,
        minimal_invoice("2.0", guiaEmbebida=obj("EmbededDespatch", partida=obj("Direction", ["150101", "P"]), pesoBruto=dec("1.5"))),
    )
    cases += with_options("inv21-empty-lists", "Listas vacías", b, minimal_invoice("2.1", details=[], legends=[], guias=[], relDocs=[], cuotas=[], cargos=[], descuentos=[], anticipos=[]), ("default",))

    # Selección de plantilla.
    for key, version in [("v22", "2.2"), ("vnull", None), ("vempty", ""), ("vspace", "2.1 "), ("v2", "2"), ("vtraversal", "2.1.xml.twig/../invoice2.0")]:
        cases.append(case(f"inv-version-{key}", f"ublVersion {version!r} no tiene plantilla", b, minimal_invoice(version)))
    return cases


# --------------------------------------------------------------------------
# Notas
# --------------------------------------------------------------------------

def note(tipo, version, **over):
    node = core.note_full(tipo, "FC01" if tipo == "07" else "FD01", "01")
    node["ublVersion"] = version
    node.update(over)
    return node


def note_cases():
    b = "NoteBuilder"
    cases = []
    for version, tag in (("2.1", "21"), ("2.0", "20")):
        cases += with_options(f"cr{tag}-full", f"Nota de crédito UBL {version} completa", b, note("07", version))
        cases += with_options(f"db{tag}-full", f"Nota de débito UBL {version} completa", b, note("08", version))
        cases += with_options(f"cr{tag}-empty", f"Nota de crédito {version} vacía", b, obj("Note", tipoDoc="07", ublVersion=version), ("default", "tests"))
        cases += with_options(f"db{tag}-empty", f"Nota de débito {version} vacía", b, obj("Note", tipoDoc="08", ublVersion=version), ("default", "tests"))
        taxes = note(
            "07",
            version,
            mtoOperGravadas=dec("0"),
            mtoOperInafectas=dec("10"),
            mtoOperExoneradas=dec("20"),
            mtoOperExportacion=dec("30"),
            mtoOperGratuitas=dec("40"),
            mtoISC=dec("1"),
            mtoIvap=dec("2"),
            mtoOtrosTributos=dec("3"),
            icbper=dec("0.4"),
            mtoIGV=dec("0"),
            details=[line("N10", "10", isc=dec("1"), tipSisIsc="02", porcentajeIsc=dec("10")), line("N20", "20", igv=dec("0")), line("N11", "11", mtoValorGratuito=dec("5")),
                     line("NOTH", "10", otroTributo=dec("3"), mtoBaseOth=dec("30"), porcentajeOth=dec("10")), line("NBOL", "10", icbper=dec("0.4"), unidad="BG"),
                     line("NATR", "10", atributos=[obj("DetailAttribute", code="1", name="Cero", value="0"), obj("DetailAttribute", code="2", name="Dur", duracion=3)])],
        )
        cases += with_options(f"cr{tag}-taxes", f"Tributos de nota de crédito {version}", b, taxes, ("default",))
        cases += with_options(f"db{tag}-taxes", f"Tributos de nota de débito {version}", b, set_fields(taxes, tipoDoc="08"), ("default",))
    cases += with_options("db21-no-totals", "Nota de débito sin valorVenta ni subTotal", b, note("08", "2.1", valorVenta=None, subTotal=None, sumOtrosCargos=None, redondeo=dec("0.01")), ("default",))
    cases += with_options("cr21-special", "Texto especial en nota de crédito", b, note("07", "2.1", desMotivo=SPECIAL, numDocfectado="F001-<1>"), ("default", "cpeforge"))
    for key, tipo in [("tipo01", "01"), ("tipo7", "7"), ("tiponull", None), ("tipo07space", "07 ")]:
        cases.append(case(f"note-selector-{key}", f"tipoDoc {tipo!r}: el selector usa la plantilla de débito", b, note("07", "2.1", tipoDoc=tipo)))
    for key, version in [("v22", "2.2"), ("vnull", None)]:
        cases.append(case(f"note-version-{key}", f"ublVersion {version!r} de nota sin plantilla", b, note("07", version)))
    cases.append(case("note-default-version", "Note sin ublVersion explícita usa 2.0", b, obj("Note", tipoDoc="07", serie="FC01")))
    return cases


# --------------------------------------------------------------------------
# Guías
# --------------------------------------------------------------------------

def despatch_cases():
    b = "DespatchBuilder"
    cases = []
    full2022 = core.despatch_full()
    legacy = core.despatch_full(version="1")
    cases += with_options("desp2022-full", "Guía 2022 completa", b, full2022)
    cases += with_options("desp-full", "Guía anterior a 2022 completa", b, legacy)
    cases += with_options("desp-version-null", "Sin versión: plantilla antigua", b, core.despatch_full(version=None), ("default",))
    cases += with_options("desp-version-space", "Versión '2022 ' no coincide exactamente", b, core.despatch_full(version="2022 "), ("default",))
    cases += with_options("desp-version-2023", "Versión 2023: plantilla antigua", b, core.despatch_full(version="2023"), ("default",))
    envio_min = obj(
        "Shipment",
        codTraslado="01",
        modTraslado="01",
        pesoTotal=dec("1"),
        undPesoTotal="KGM",
        numBultos=0,
        llegada=obj("Direction", ["150101", "LLEGADA"]),
        partida=obj("Direction", ["150102", "PARTIDA"]),
    )
    minimal = core.despatch_full(
        version="2022",
        observacion=None,
        tercero=None,
        comprador=None,
        docBaja=None,
        relDoc=None,
        addDocs=[],
        envio=envio_min,
        details=[obj("DespatchDetail", codigo="P1", descripcion="X", unidad="NIU", cantidad=dec("1.5"))],
    )
    cases += with_options("desp2022-minimal", "Guía 2022 mínima: sin fechas de traslado, vehículo ni conductores", b, minimal, ("default", "tests"))
    cases += with_options("desp-minimal", "Guía antigua mínima sin transportista", b, set_fields(minimal, version=None), ("default", "tests"))
    airport = core.despatch_full(
        envio=set_fields(
            core.shipment(),
            puerto=None,
            indTransbordo=True,
            llegada=obj("Direction", ["150101", "LLEGADA"], codLocal="0003"),
            partida=obj("Direction", ["150102", "PARTIDA"]),
        ),
        addDocs=[obj("AdditionalDoc", tipoDesc="Factura", tipo="01", nro="F1"), obj("AdditionalDoc", tipo="49", nro="")],
    )
    cases += with_options("desp2022-airport", "Aeropuerto (elseif), códigos de local parciales y documentos sin emisor", b, airport, ("default",))
    cases += with_options("desp-airport", "Transbordo verdadero en plantilla antigua", b, set_fields(airport, version=None), ("default",))
    cases += with_options(
        "desp2022-null-nro",
        "AdditionalDoc con nro nulo: getNro lanza TypeError durante el render",
        b,
        core.despatch_full(addDocs=[obj("AdditionalDoc", tipo="01")]),
        ("default", "tests"),
    )
    cases += with_options(
        "desp2022-quantities",
        "Cantidades con n_format_limit(10) y pesos con n_format(3)",
        b,
        core.despatch_full(details=[obj("DespatchDetail", codigo="Q", descripcion="Q", unidad="KGM", cantidad=dec(v)) for v in ("1", "0.12345678901", "1e-6", "123.4567")]),
        ("default",),
    )
    cases += with_options("desp2022-special", "Texto especial en guía 2022", b, core.despatch_full(observacion=SPECIAL, destinatario=core.client(rznSocial=SPECIAL)), ("default", "cpeforge"))
    cases += with_options("desp-empty", "Despatch vacío: plantilla antigua", b, obj("Despatch"))
    cases += with_options("desp2022-empty", "Despatch 2022 vacío", b, obj("Despatch", version="2022"))
    return cases


# --------------------------------------------------------------------------
# Resumen y bajas
# --------------------------------------------------------------------------

def summary_cases():
    b = "SummaryBuilder"
    cases = []
    full = obj(
        "Summary",
        correlativo="001",
        fecGeneracion=dt("2024-01-31T20:00:00-05:00"),
        fecResumen=dt("2024-02-01T03:00:00+00:00", "UTC"),
        moneda="PEN",
        company=core.company(),
        details=[
            core.summary_detail(),
            obj("SummaryDetail", tipoDoc="03", serieNro="B001-2", clienteTipo="1", clienteNro="1", estado="1", total=dec("10"), mtoOperGravadas=dec("0"), mtoIvap=dec("1.5")),
            obj("SummaryDetail", tipoDoc="07", serieNro="BC01-1", clienteTipo="1", clienteNro="1", estado="3", total=dec("5"), docReferencia=obj("Document", tipoDoc="03", nroDoc="B001-1"), mtoIGV=dec("0.9"), porcentajeIgv=dec("18"), mtoISC=dec("0.5"), mtoOtrosTributos=dec("0.2")),
        ],
    )
    cases += with_options("summary-full", "Resumen diario con IVAP, IGV, ISC, otros, ICBPER y percepción", b, full)
    cases += with_options("summary-usd", "Resumen en USD", b, set_fields(full, moneda="USD"), ("default",))
    cases += with_options("summary-null-generation", "fecGeneracion nula usa la hora actual", b, set_fields(full, fecGeneracion=None), ("default",))
    cases += with_options("summary-null-resumen", "fecResumen nula: getXmlId lanza TypeError", b, set_fields(full, fecResumen=None), ("default", "tests"))
    cases += with_options("summary-empty-details", "Sin líneas", b, set_fields(full, details=[]), ("default",))
    cases += with_options("summary-no-company", "Sin emisor", b, set_fields(full, company=None))
    cases += with_options("summary-empty-line", "Línea vacía: mtoIGV nulo en n_format", b, set_fields(full, details=[obj("SummaryDetail")]))
    return cases


def voided_cases():
    cases = []
    detail = obj("VoidedDetail", tipoDoc="01", serie="F001", correlativo="123", desMotivoBaja="ERROR " + SPECIAL)
    voided = obj(
        "Voided",
        correlativo="00001",
        fecGeneracion=dt("2024-01-30T10:00:00-05:00"),
        fecComunicacion=dt("2024-02-01T04:59:59+00:00", "UTC"),
        company=core.company(),
        details=[detail, obj("VoidedDetail", tipoDoc="03", serie="B001", correlativo="9", desMotivoBaja="x>  <y")],
    )
    cases += with_options("voided-full", "Comunicación de baja RA", "VoidedBuilder", voided)
    reversion = copy.deepcopy(voided)
    reversion["$class"] = "Reversion"
    cases += with_options("reversion-full", "Resumen de reversiones RR", "VoidedBuilder", reversion)
    cases += with_options("voided-empty-details", "Baja sin líneas", "VoidedBuilder", set_fields(voided, details=[]), ("default",))
    cases += with_options("voided-null-date", "fecComunicacion nula: error", "VoidedBuilder", set_fields(voided, fecComunicacion=None), ("default", "tests"))
    cases += with_options("voided-null-generation", "fecGeneracion nula usa la hora actual", "VoidedBuilder", set_fields(voided, fecGeneracion=None), ("default",))
    cases += with_options("voided-no-company", "Baja sin emisor", "VoidedBuilder", set_fields(voided, company=None))
    return cases


# --------------------------------------------------------------------------
# Retención y percepción
# --------------------------------------------------------------------------

def retention_cases():
    cases = []
    for name, builder, maker, detail_class, amount_a, amount_b, date_field, pay_field in (
        ("retention", "RetentionBuilder", core.retention_full, "RetentionDetail", "impRetenido", "impPagar", "fechaRetencion", "pagos"),
        ("perception", "PerceptionBuilder", core.perception_full, "PerceptionDetail", "impPercibido", "impCobrar", "fechaPercepcion", "cobros"),
    ):
        full = maker(proveedor=core.client(rznSocial=SPECIAL))
        cases += with_options(f"{name}-full", f"{name} completa con nombre especial del receptor", builder, full)
        partial = maker(
            observacion=None,
            details=[
                obj(detail_class, tipoDoc="01", numDoc="F1", fechaEmision=dt("2024-04-01T02:00:00+00:00", "UTC"), impTotal=dec("10"), moneda="PEN"),
                obj(detail_class, tipoDoc="01", numDoc="F2", fechaEmision=dt("2024-04-01T10:00:00-05:00"), impTotal=dec("10"), moneda="PEN",
                    **{amount_a: dec("0"), amount_b: dec("10"), date_field: dt("2024-04-02T10:00:00-05:00"), pay_field: []}),
                obj(detail_class, tipoDoc="01", numDoc="F3", fechaEmision=dt("2024-04-01T10:00:00-05:00"), impTotal=dec("10"), moneda="USD",
                    **{amount_a: dec("1"), amount_b: dec("9"), date_field: dt("2024-04-02T10:00:00-05:00"),
                       "tipoCambio": obj("Exchange", monedaRef="USD", monedaObj="PEN", factor=dec("3.7123456789"), fecha=dt("2024-04-02T00:00:00-05:00"))}),
            ],
        )
        cases += with_options(f"{name}-partial", f"{name}: detalles sin pagos, información incompleta e importe 0", builder, partial, ("default", "tests"))
        cases += with_options(f"{name}-empty", f"{name} vacía", builder, obj("Retention" if name == "retention" else "Perception"))
        cases += with_options(f"{name}-no-provider", f"{name} sin proveedor", builder, maker(proveedor=None))
    return cases


FAMILIES = {
    "invoice": invoice_cases,
    "note": note_cases,
    "despatch": despatch_cases,
    "summary": summary_cases,
    "voided": voided_cases,
    "retention-perception": retention_cases,
}


def filter_inputs():
    values = [
        None, "0", "-0.00", "0.004", "-0.004", "-0.005", "0.005", "1.005", "2.675", "0.285", "1.0049999999999999", "10.4",
        "17.22111", "20.677722", "30.22", "1234567.891", "99999999999.995", "1e15", "1e14", "99999999999999", "123456789012345.67",
        "0.1", "0.0001", "0.00001", "1e-6", "10.343434223423423", "432.23", "59.0", "0.123456789012345", "123456789012345",
        "1.5", "2.5", "-1.5", "1e20", "0.30000000000000004", "999999999999999.5", "3.7123456789", "12345678901234567.89",
    ]
    inputs = []
    for value in values:
        node = None if value is None else dec(value)
        inputs.append({"fn": "print", "value": node})
        inputs.append({"fn": "n_format", "value": node})
        for decimals in (0, 3, 6):
            inputs.append({"fn": "n_format", "value": node, "decimals": decimals})
        for decimals in (5, 10):
            inputs.append({"fn": "n_format_limit", "value": node, "decimals": decimals})
    for code in ("10", "11", "17", "20", "21", "30", "40", "99", None, "", "10abc", " 20", "x", "1e1", "40.9", "-10"):
        inputs.append({"fn": "tributo_afectacion", "value": code})
    for code in ("1000", "1016", "2000", "9995", "9996", "9997", "9998", "9999", "2", "7152", None, ""):
        inputs.append({"fn": "tributo", "value": code})
    return inputs


def main() -> None:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "tests/reference/xml"
    (root / "cases").mkdir(parents=True, exist_ok=True)
    seen: set[str] = set()
    for family, build in FAMILIES.items():
        cases = build()
        for c in cases:
            if c["id"] in seen:
                raise SystemExit(f"id duplicado: {c['id']}")
            seen.add(c["id"])
        text = json.dumps({"family": family, "cases": cases}, ensure_ascii=False, indent=2) + "\n"
        (root / "cases" / f"{family}.json").write_text(text, encoding="utf-8")
    (root / "filters").mkdir(parents=True, exist_ok=True)
    text = json.dumps({"inputs": filter_inputs()}, ensure_ascii=False, indent=2) + "\n"
    (root / "filters" / "inputs.json").write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
