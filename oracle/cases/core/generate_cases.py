#!/usr/bin/env python3
"""Genera los casos sintéticos de paridad de greenter/core.

Los casos son entradas neutrales (sin resultados esperados): el oráculo PHP
(`oracle/php/core/run_cases.php`) y el arnés Rust (`crates/core/tests/parity.rs`)
los ejecutan por separado. Los datos son ficticios; los RUC y documentos no
corresponden a contribuyentes reales.

Uso: python3 oracle/cases/core/generate_cases.py [directorio_salida]
"""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path


def dec(text: str) -> dict:
    return {"$decimal": text}


def dt(instant: str, tz: str = "America/Lima", cls: str = "DateTimeImmutable") -> dict:
    return {"$datetime": instant, "tz": tz, "class": cls}


def raw(data: bytes) -> dict:
    return {"$bytes": base64.b64encode(data).decode("ascii")}


def obj(cls: str, ctor: list | None = None, **fields) -> dict:
    node: dict = {"$class": cls}
    if ctor is not None:
        node["$ctor"] = ctor
    node.update(fields)
    return node


def case(case_id: str, description: str, node: dict, calls: list[str] | None = None) -> dict:
    out = {"id": case_id, "description": description, "input": node}
    if calls:
        out["calls"] = calls
    return out


# --------------------------------------------------------------------------
# Identidad
# --------------------------------------------------------------------------

def address(**over) -> dict:
    fields = dict(
        ubigueo="150101",
        departamento="LIMA",
        provincia="LIMA",
        distrito="LIMA",
        urbanizacion="CERCADO",
        direccion="AV. FICTICIA 123",
        codigoPais="PE",
        codLocal="0001",
    )
    fields.update(over)
    return obj("Address", **fields)


def company(**over) -> dict:
    fields = dict(
        ruc="20000000001",
        razonSocial="EMPRESA SINTETICA S.A.C.",
        nombreComercial="SINTETICA",
        address=address(),
        email="facturacion@example.test",
        telephone="01-0000000",
    )
    fields.update(over)
    return obj("Company", **fields)


def client(**over) -> dict:
    fields = dict(
        tipoDoc="6",
        numDoc="20000000002",
        rznSocial="CLIENTE SINTETICO S.A.",
        address=address(ubigueo="150122", direccion="CALLE PRUEBA 456"),
        email="cliente@example.test",
        telephone="999000000",
    )
    fields.update(over)
    return obj("Client", **fields)


def identity_cases() -> list[dict]:
    return [
        case("address-defaults", "Address recién creado: codigoPais=PE y codLocal=0000 por defecto", obj("Address")),
        case("address-full", "Address con todos los campos", address()),
        case(
            "address-null-over-defaults",
            "Establecer null sobre valores por defecto los reemplaza por null",
            obj("Address", codigoPais=None, codLocal=None),
        ),
        case(
            "address-empty-strings",
            "Cadenas vacías se conservan y no equivalen a null",
            obj("Address", ubigueo="", codigoPais="", codLocal="", direccion=""),
        ),
        case("company-empty", "Company sin datos", obj("Company")),
        case("company-full", "Company con dirección anidada", company()),
        case("client-empty", "Client sin datos", obj("Client")),
        case("client-full", "Client con dirección anidada", client()),
        case(
            "client-unicode",
            "Texto Unicode, comillas y caracteres de control comunes se conservan",
            client(rznSocial="ÑANDÚ & CÍA \"LOS ÁLAMOS\" <S.R.L.>", email="ñ@example.test", telephone="\t01\n"),
        ),
    ]


# --------------------------------------------------------------------------
# Ventas
# --------------------------------------------------------------------------

def charge(cod: str, factor: str, monto: str, base: str) -> dict:
    return obj("Charge", codTipo=cod, factor=dec(factor), monto=dec(monto), montoBase=dec(base))


def attribute() -> dict:
    return obj(
        "DetailAttribute",
        code="5010",
        name="Numero de placa",
        value="ABC-123",
        fecInicio=dt("2024-03-01T08:00:00-05:00", cls="DateTime"),
        fecFin=dt("2024-03-02T08:00:00-05:00", cls="DateTime"),
        duracion=2,
    )


def sale_detail(code: str = "P001", **over) -> dict:
    fields = dict(
        unidad="NIU",
        cantidad=dec("2"),
        codProducto=code,
        codProdSunat="10000000",
        codProdGS1="7750000000000",
        descripcion="PRODUCTO SINTETICO " + code,
        mtoValorUnitario=dec("50.00"),
        cargos=[charge("47", "0.10", "10.00", "100.00")],
        descuentos=[charge("00", "0.05", "5.00", "100.00")],
        descuento=dec("5.00"),
        mtoBaseIgv=dec("100.00"),
        porcentajeIgv=dec("18.00"),
        igv=dec("18.00"),
        tipAfeIgv="10",
        mtoBaseIsc=dec("0.00"),
        porcentajeIsc=dec("0.00"),
        isc=dec("0.00"),
        tipSisIsc="01",
        mtoBaseOth=dec("0.00"),
        porcentajeOth=dec("0.00"),
        otroTributo=dec("0.00"),
        icbper=dec("0.80"),
        factorIcbper=dec("0.40"),
        totalImpuestos=dec("18.80"),
        mtoPrecioUnitario=dec("59.00"),
        mtoValorVenta=dec("100.00"),
        mtoValorGratuito=dec("0.00"),
        atributos=[attribute()],
    )
    fields.update(over)
    return obj("SaleDetail", **fields)


def sale_base_fields(tipo_doc: str, serie: str, correlativo: str) -> dict:
    return dict(
        ublVersion="2.1",
        tipoDoc=tipo_doc,
        serie=serie,
        correlativo=correlativo,
        fechaEmision=dt("2024-01-31T23:30:00-05:00"),
        company=company(),
        client=client(),
        tipoMoneda="PEN",
        sumOtrosCargos=dec("1.00"),
        mtoOperGravadas=dec("200.00"),
        mtoOperInafectas=dec("0.00"),
        mtoOperExoneradas=dec("0.00"),
        mtoOperExportacion=dec("0.00"),
        mtoOperGratuitas=dec("0.00"),
        mtoIGVGratuitas=dec("0.00"),
        mtoIGV=dec("36.00"),
        mtoBaseIvap=dec("0.00"),
        mtoIvap=dec("0.00"),
        mtoBaseIsc=dec("0.00"),
        mtoISC=dec("0.00"),
        mtoBaseOth=dec("0.00"),
        mtoOtrosTributos=dec("0.00"),
        icbper=dec("1.60"),
        totalImpuestos=dec("37.60"),
        redondeo=dec("-0.01"),
        mtoImpVenta=dec("237.59"),
        details=[sale_detail("P001"), sale_detail("P002", cantidad=dec("1.5"))],
        legends=[obj("Legend", code="1000", value="SON DOSCIENTOS TREINTA Y SIETE CON 59/100 SOLES")],
        guias=[obj("Document", tipoDoc="09", nroDoc="T001-1")],
        relDocs=[obj("Document", tipoDoc="99", nroDoc="X-1")],
        compra="OC-0001",
        formaPago=obj("FormaPagoCredito", [dec("237.59"), "PEN"]),
        cuotas=[
            obj("Cuota", moneda="PEN", monto=dec("100.00"), fechaPago=dt("2024-02-29T12:00:00-05:00")),
            obj("Cuota", moneda="PEN", monto=dec("137.59"), fechaPago=dt("2024-03-31T12:00:00-05:00")),
        ],
    )


def embeded_despatch() -> dict:
    return obj(
        "EmbededDespatch",
        llegada=obj("Direction", ["150101", "AV. LLEGADA 1"], codLocal="0002", ruc="20000000001"),
        partida=obj("Direction", ["150203", "AV. PARTIDA 2"]),
        transportista=client(tipoDoc="6", numDoc="20000000003", rznSocial="TRANSPORTES SINTETICOS"),
        nroLicencia="Q00000000",
        transpPlaca="ABC-123",
        transpCodeAuth="AUTH-1",
        transpMarca="MARCA",
        modTraslado="01",
        pesoBruto=dec("12.500"),
        undPesoBruto="KGM",
    )


def invoice_full() -> dict:
    fields = sale_base_fields("01", "F001", "123")
    fields.update(
        tipoOperacion="0101",
        fecVencimiento=dt("2024-03-31T00:00:00-05:00"),
        sumDsctoGlobal=dec("2.00"),
        mtoDescuentos=dec("3.00"),
        sumOtrosDescuentos=dec("4.00"),
        descuentos=[charge("02", "0.01", "2.00", "200.00")],
        cargos=[charge("50", "0.02", "4.00", "200.00")],
        mtoCargos=dec("4.00"),
        totalAnticipos=dec("10.00"),
        perception=obj(
            "SalePerception", codReg="01", porcentaje=dec("0.02"), mtoBase=dec("237.59"), mto=dec("4.75"), mtoTotal=dec("242.34")
        ),
        guiaEmbebida=embeded_despatch(),
        anticipos=[obj("Prepayment", tipoDocRel="02", nroDocRel="F001-100", total=dec("10.00"))],
        detraccion=obj(
            "Detraction",
            percent=dec("12.00"),
            mount=dec("28.51"),
            ctaBanco="00-000-000000",
            codMedioPago="001",
            codBienDetraccion="037",
            valueRef=dec("100.00"),
        ),
        seller=client(tipoDoc="1", numDoc="00000001", rznSocial="VENDEDOR SINTETICO"),
        valorVenta=dec("200.00"),
        subTotal=dec("237.60"),
        observacion="Observación de prueba",
        direccionEntrega=address(direccion="ENTREGA 789"),
    )
    return obj("Invoice", **fields)


def note_full(tipo_doc: str, serie: str, cod_motivo: str) -> dict:
    fields = sale_base_fields(tipo_doc, serie, "45")
    fields.update(
        codMotivo=cod_motivo,
        desMotivo="MOTIVO SINTETICO",
        tipDocAfectado="01",
        numDocfectado="F001-123",
        perception=obj("SalePerception", codReg="02", porcentaje=dec("0.01"), mtoBase=dec("10"), mto=dec("0.10"), mtoTotal=dec("10.10")),
        valorVenta=dec("200.00"),
        subTotal=dec("236.00"),
    )
    return obj("Note", **fields)


def receipt_full(**over) -> dict:
    fields = dict(
        person=company(ruc="10000000001", razonSocial="PERSONA NATURAL SINTETICA"),
        receptor=client(),
        serie="E001",
        correlativo="7",
        fechaEmision=dt("2024-05-01T10:00:00-05:00"),
        concepto="SERVICIO PROFESIONAL",
        montoLetras="MIL CON 00/100 SOLES",
        subTotal=dec("1000.00"),
        retencion=dec("80.00"),
        porcentaje=dec("8.00"),
        total=dec("920.00"),
    )
    fields.update(over)
    return obj("Receipt", **fields)


def sale_cases() -> list[dict]:
    name = ["getName"]
    return [
        case("base-sale-empty", "BaseSale es una clase concreta: ublVersion=2.0", obj("BaseSale")),
        case(
            "base-sale-full",
            "BaseSale implementa DocumentInterface por sí misma",
            obj("BaseSale", **sale_base_fields("01", "F002", "9")),
            name,
        ),
        case("invoice-empty", "Invoice recién creado: ublVersion=2.0 y el resto null", obj("Invoice")),
        case("invoice-empty-name", "getName sin company falla en PHP", obj("Invoice"), name),
        case("invoice-full", "Factura con todos los campos, varias líneas, cargos y descuentos", invoice_full(), name),
        case(
            "invoice-boleta-name",
            "Boleta (03) con serie B001",
            obj("Invoice", tipoDoc="03", serie="B001", correlativo="1", company=company()),
            name,
        ),
        case(
            "invoice-name-null-parts",
            "Partes nulas del nombre se unen como cadena vacía",
            obj("Invoice", company=obj("Company")),
            name,
        ),
        case(
            "invoice-name-empty-ruc",
            "RUC vacío y correlativo con ceros a la izquierda",
            obj("Invoice", tipoDoc="01", serie="F001", correlativo="00000001", company=company(ruc="")),
            name,
        ),
        case(
            "invoice-name-unicode",
            "Serie y correlativo con caracteres no ASCII se concatenan sin validar",
            obj("Invoice", tipoDoc="01", serie="FÑ01", correlativo="1-2", company=company()),
            name,
        ),
        case("invoice-forma-pago-contado", "Forma de pago al contado", obj("Invoice", formaPago=obj("FormaPagoContado"))),
        case(
            "invoice-forma-pago-credito-default",
            "Forma de pago a crédito sin monto ni moneda",
            obj("Invoice", formaPago=obj("FormaPagoCredito")),
        ),
        case(
            "invoice-forma-pago-generic",
            "PaymentTerms genérico con setters",
            obj("Invoice", formaPago=obj("PaymentTerms", tipo="Otro", moneda="USD", monto=dec("0.00"))),
        ),
        case(
            "invoice-empty-lists",
            "Listas vacías se distinguen de null",
            obj("Invoice", details=[], legends=[], guias=[], relDocs=[], cuotas=[], descuentos=[], cargos=[], anticipos=[]),
        ),
        case("note-empty", "Note recién creada", obj("Note")),
        case("note-credit-full", "Nota de crédito (07) completa", note_full("07", "FC01", "01"), name),
        case("note-debit-full", "Nota de débito (08) completa", note_full("08", "FD01", "02"), name),
        case("note-empty-name", "getName de Note sin company falla", obj("Note", serie="FC01"), name),
        case("receipt-empty", "Recibo por honorarios vacío", obj("Receipt")),
        case("receipt-full", "Recibo por honorarios: nombre RHE + RUC + correlativo (sin serie)", receipt_full(), name),
        case("receipt-name-no-person", "getName sin persona falla en PHP", receipt_full(person=None), name),
        case(
            "receipt-name-null-parts",
            "RUC y correlativo nulos producen solo el prefijo",
            obj("Receipt", person=obj("Company")),
            name,
        ),
        case("sale-detail-defaults", "SaleDetail: factorIcbper=0.3 por defecto", obj("SaleDetail")),
        case("sale-detail-null-factor", "factorIcbper puede ponerse en null", obj("SaleDetail", factorIcbper=None)),
        case("detail-attribute-empty", "DetailAttribute vacío", obj("DetailAttribute")),
        case(
            "detail-attribute-mutable-dates",
            "DetailAttribute acepta DateTime mutable",
            attribute(),
        ),
        case("charge-empty", "Charge vacío", obj("Charge")),
        case("cuota-empty", "Cuota vacía", obj("Cuota")),
        case("detraction-empty", "Detraction vacía", obj("Detraction")),
        case("document-empty", "Document vacío", obj("Document")),
        case("embeded-despatch-full", "Guía embebida completa", embeded_despatch()),
        case("legend-empty", "Legend vacía", obj("Legend")),
        case("prepayment-empty", "Prepayment vacío", obj("Prepayment")),
        case("sale-perception-empty", "SalePerception vacía", obj("SalePerception")),
        case("payment-terms-empty", "PaymentTerms vacío", obj("PaymentTerms")),
        case("forma-pago-contado", "FormaPagoContado fija tipo=Contado", obj("FormaPagoContado")),
        case(
            "forma-pago-contado-overridden",
            "Los setters pueden cambiar lo fijado por el constructor",
            obj("FormaPagoContado", tipo="Credito", monto=dec("5"), moneda="PEN"),
        ),
        case("forma-pago-credito-full", "FormaPagoCredito con monto y moneda", obj("FormaPagoCredito", [dec("1500.75"), "USD"])),
        case("forma-pago-credito-monto-only", "FormaPagoCredito solo con monto", obj("FormaPagoCredito", [dec("10")])),
    ]


# --------------------------------------------------------------------------
# Guías de remisión
# --------------------------------------------------------------------------

def vehicle(placa: str, secundarios: list | None = None) -> dict:
    fields = dict(placa=placa, nroCirculacion="CIRC-" + placa, codEmisor="MTC", nroAutorizacion="AUT-" + placa)
    if secundarios is not None:
        fields["secundarios"] = secundarios
    return obj("Vehicle", **fields)


def shipment() -> dict:
    return obj(
        "Shipment",
        codTraslado="01",
        desTraslado="VENTA",
        sustentoPeso="Sustento",
        indTransbordo=False,
        indicadores=["SUNAT_Envio_IndicadorTrasladoVehiculoM1L"],
        pesoItems=dec("10.5"),
        pesoTotal=dec("12.500"),
        undPesoTotal="KGM",
        numBultos=3,
        modTraslado="02",
        fecTraslado=dt("2024-12-31T23:59:59-05:00", cls="DateTime"),
        fecEntregaBienes=dt("2025-01-01T08:00:00-05:00", cls="DateTime"),
        numContenedor="CONT-1",
        contenedores=["CONT-1", "CONT-2"],
        codPuerto="PECLL",
        puerto=obj("Puerto", codigo="PECLL", nombre="CALLAO"),
        aeropuerto=obj("Puerto", codigo="LIM", nombre="JORGE CHAVEZ"),
        transportista=obj(
            "Transportist",
            tipoDoc="6",
            numDoc="20000000003",
            rznSocial="TRANSPORTES SINTETICOS",
            nroMtc="MTC-1",
            placa="ABC-123",
            choferTipoDoc="1",
            choferDoc="00000002",
        ),
        vehiculo=vehicle("ABC-123", [vehicle("DEF-456", [vehicle("GHI-789")])]),
        choferes=[
            obj("Driver", tipo="Principal", tipoDoc="1", nroDoc="00000002", nombres="JUAN", apellidos="PRUEBA", licencia="Q0000002"),
            obj("Driver", tipo="Secundario", tipoDoc="1", nroDoc="00000003", nombres="ANA", apellidos="PRUEBA", licencia="Q0000003"),
        ],
        llegada=obj("Direction", ["150101", "AV. LLEGADA 1"], codLocal="0002", ruc="20000000001"),
        partida=obj("Direction", ["150203", "AV. PARTIDA 2"], codLocal="0001", ruc="20000000001"),
    )


def despatch_full(**over) -> dict:
    fields = dict(
        version="2022",
        tipoDoc="09",
        serie="T001",
        correlativo="12",
        observacion="Guía sintética",
        fechaEmision=dt("2024-06-15T10:00:00-05:00"),
        company=company(),
        destinatario=client(),
        tercero=client(numDoc="20000000004"),
        comprador=client(numDoc="20000000005"),
        envio=shipment(),
        docBaja=obj("Document", tipoDoc="09", nroDoc="T001-1"),
        relDoc=obj("Document", tipoDoc="01", nroDoc="F001-1"),
        addDocs=[obj("AdditionalDoc", tipoDesc="Factura", tipo="01", nro="F001-1", emisor="20000000001")],
        details=[
            obj(
                "DespatchDetail",
                codigo="P001",
                descripcion="ITEM 1",
                unidad="NIU",
                cantidad=dec("2"),
                codProdSunat="10000000",
                atributos=[obj("DetailAttribute", code="7000", name="Partida", value="0000")],
            ),
            obj("DespatchDetail", codigo="P002", descripcion="ITEM 2", unidad="KGM", cantidad=dec("0.125")),
        ],
    )
    fields.update(over)
    return obj("Despatch", **fields)


def despatch_cases() -> list[dict]:
    name = ["getName"]
    return [
        case("despatch-empty", "Despatch vacío", obj("Despatch")),
        case("despatch-full", "Guía de remisión completa con envío, vehículos anidados y choferes", despatch_full(), name),
        case("despatch-carrier-name", "Guía transportista (31)", despatch_full(tipoDoc="31", serie="V001"), name),
        case("despatch-name-no-company", "getName sin company falla", obj("Despatch", tipoDoc="09"), name),
        case("despatch-name-null-parts", "Partes nulas del nombre", obj("Despatch", company=obj("Company")), name),
        case("shipment-empty", "Shipment vacío", obj("Shipment")),
        case("shipment-full", "Shipment completo", shipment()),
        case("shipment-transbordo-true", "indTransbordo true y bultos 0", obj("Shipment", indTransbordo=True, numBultos=0)),
        case("direction-ctor-null", "Direction requiere dos argumentos que pueden ser null", obj("Direction", [None, None])),
        case("direction-ctor-values", "Direction con ubigeo y dirección", obj("Direction", ["150101", "AV. 1"])),
        case("vehicle-empty", "Vehicle vacío", obj("Vehicle")),
        case("vehicle-empty-secundarios", "Vehicle con lista vacía de secundarios", obj("Vehicle", secundarios=[])),
        case("driver-empty", "Driver vacío", obj("Driver")),
        case("puerto-empty", "Puerto vacío", obj("Puerto")),
        case("transportist-empty", "Transportist vacío", obj("Transportist")),
        case("despatch-detail-empty", "DespatchDetail vacío", obj("DespatchDetail")),
        case(
            "additional-doc-full",
            "AdditionalDoc completo",
            obj("AdditionalDoc", tipoDesc="Factura", tipo="01", nro="F001-1", emisor="20000000001"),
            ["getNro"],
        ),
        case(
            "additional-doc-empty",
            "getNro declara retorno string no nullable: con nro null PHP lanza TypeError",
            obj("AdditionalDoc"),
            ["getNro"],
        ),
        case(
            "additional-doc-empty-nro",
            "nro vacío es válido para el getter",
            obj("AdditionalDoc", nro=""),
            ["getNro"],
        ),
        case(
            "despatch-additional-doc-null-nro",
            "AdditionalDoc anidado con nro null",
            obj("Despatch", addDocs=[obj("AdditionalDoc", tipo="01")]),
        ),
    ]


# --------------------------------------------------------------------------
# Resúmenes y bajas (fechas Perú)
# --------------------------------------------------------------------------

def summary_detail() -> dict:
    return obj(
        "SummaryDetail",
        tipoDoc="03",
        serieNro="B001-1",
        clienteTipo="1",
        clienteNro="00000001",
        docReferencia=obj("Document", tipoDoc="03", nroDoc="B001-0"),
        percepcion=obj("SummaryPerception", codReg="01", tasa=dec("2.00"), mtoBase=dec("100"), mto=dec("2"), mtoTotal=dec("102")),
        estado="1",
        total=dec("118.00"),
        porcentajeIgv=dec("18.00"),
        mtoOperGravadas=dec("100.00"),
        mtoOperInafectas=dec("0"),
        mtoOperExoneradas=dec("0"),
        mtoOperExportacion=dec("0"),
        mtoOperGratuitas=dec("0"),
        mtoOtrosCargos=dec("0"),
        mtoIGV=dec("18.00"),
        mtoIvap=dec("0"),
        mtoISC=dec("0"),
        mtoOtrosTributos=dec("0"),
        mtoIcbper=dec("0.50"),
    )


SUMMARY_DATES = [
    ("lima-civil", "Fecha civil en Lima", "2024-01-31T23:30:00-05:00", "America/Lima", "DateTimeImmutable"),
    ("utc-previous-day", "03:00 UTC es el día anterior en Lima", "2024-02-01T03:00:00+00:00", "UTC", "DateTimeImmutable"),
    ("utc-exact-midnight", "05:00 UTC es medianoche en Lima", "2024-02-01T05:00:00+00:00", "UTC", "DateTimeImmutable"),
    ("utc-one-second-before", "04:59:59 UTC sigue en el día anterior en Lima", "2024-02-01T04:59:59+00:00", "UTC", "DateTimeImmutable"),
    ("leap-day", "Año bisiesto: 29 de febrero", "2024-03-01T02:00:00+00:00", "UTC", "DateTimeImmutable"),
    ("year-boundary", "Cambio de año al convertir a Lima", "2025-01-01T04:59:59.999999+00:00", "UTC", "DateTimeImmutable"),
    ("month-boundary", "Cambio de mes al convertir a Lima", "2024-05-01T00:30:00+00:00", "UTC", "DateTimeImmutable"),
    ("fixed-offset-east", "Zona de desfase fijo +09:00", "2024-07-01T08:00:00+09:00", "+09:00", "DateTimeImmutable"),
    ("named-zone-madrid", "Otra zona IANA con horario de verano", "2024-07-01T01:30:00+02:00", "Europe/Madrid", "DateTimeImmutable"),
    ("mutable-datetime", "DateTime mutable: getXmlId no debe alterar la fecha original", "2024-02-01T03:00:00+00:00", "UTC", "DateTime"),
    ("lima-dst-1994", "Horario de verano histórico de Lima (1994, UTC-4)", "1994-02-15T04:30:00+00:00", "UTC", "DateTimeImmutable"),
    ("lima-lmt-1900", "Hora media local de Lima antes de 1908", "1900-01-01T05:05:00+00:00", "UTC", "DateTimeImmutable"),
    ("late-year-9999", "Fecha tardía del año 9999 dentro del rango Rust", "9999-12-30T12:00:00-05:00", "America/Lima", "DateTimeImmutable"),
    ("max-four-digit-year", "Último segundo del año 9999 en Lima", "9999-12-31T23:59:59-05:00", "America/Lima", "DateTimeImmutable"),
    ("year-0999", "Año con menos de cuatro cifras", "0999-06-15T12:00:00+00:00", "UTC", "DateTimeImmutable"),
    ("microseconds", "Microsegundos se conservan", "2024-02-01T04:59:59.123456+00:00", "UTC", "DateTimeImmutable"),
]


def summary_cases() -> list[dict]:
    calls = ["getXmlId", "getName"]
    cases = [
        case("summary-empty", "Summary vacío: moneda=PEN por defecto", obj("Summary")),
        case("summary-empty-calls", "getXmlId sin fecResumen lanza TypeError", obj("Summary"), calls),
        case(
            "summary-full",
            "Resumen diario completo",
            obj(
                "Summary",
                correlativo="001",
                fecGeneracion=dt("2024-01-31T20:00:00-05:00"),
                fecResumen=dt("2024-01-31T23:30:00-05:00"),
                moneda="USD",
                company=company(),
                details=[summary_detail(), obj("SummaryDetail", tipoDoc="07", estado="3")],
            ),
            calls,
        ),
        case(
            "summary-no-company",
            "getXmlId funciona sin company; getName falla",
            obj("Summary", correlativo="2", fecResumen=dt("2024-01-31T23:30:00-05:00")),
            calls,
        ),
        case(
            "summary-null-correlativo",
            "Correlativo null produce un sufijo vacío",
            obj("Summary", fecResumen=dt("2024-01-31T23:30:00-05:00"), company=company()),
            calls,
        ),
        case("summary-detail-empty", "SummaryDetail vacío", obj("SummaryDetail")),
        case("summary-detail-full", "SummaryDetail completo", summary_detail()),
        case("summary-perception-empty", "SummaryPerception vacía", obj("SummaryPerception")),
    ]
    for key, desc, instant, tz, cls in SUMMARY_DATES:
        cases.append(
            case(
                f"summary-date-{key}",
                desc,
                obj("Summary", correlativo="1", fecResumen=dt(instant, tz, cls), company=company()),
                calls,
            )
        )
    return cases


def voided_cases() -> list[dict]:
    calls = ["getXmlId", "getName"]
    detail = obj("VoidedDetail", tipoDoc="01", serie="F001", correlativo="123", desMotivoBaja="ERROR EN RUC")
    cases = [
        case("voided-empty", "Voided vacío", obj("Voided")),
        case("voided-empty-calls", "getXmlId sin fecComunicacion lanza TypeError", obj("Voided"), calls),
        case(
            "voided-full",
            "Comunicación de baja (RA) completa",
            obj(
                "Voided",
                correlativo="00001",
                fecGeneracion=dt("2024-01-30T10:00:00-05:00"),
                fecComunicacion=dt("2024-01-31T23:30:00-05:00"),
                company=company(),
                details=[detail, obj("VoidedDetail")],
            ),
            calls,
        ),
        case(
            "voided-no-company",
            "getXmlId funciona sin company; getName falla",
            obj("Voided", correlativo="1", fecComunicacion=dt("2024-01-31T23:30:00-05:00")),
            calls,
        ),
        case("reversion-empty", "Reversion vacía", obj("Reversion")),
        case("reversion-empty-calls", "getXmlId de Reversion sin fecha falla", obj("Reversion"), calls),
        case(
            "reversion-full",
            "Resumen de reversiones (RR) completo",
            obj(
                "Reversion",
                correlativo="00002",
                fecGeneracion=dt("2024-01-30T10:00:00-05:00"),
                fecComunicacion=dt("2024-02-01T03:00:00+00:00", "UTC"),
                company=company(),
                details=[obj("VoidedDetail", tipoDoc="20", serie="R001", correlativo="5", desMotivoBaja="ANULACION")],
            ),
            calls,
        ),
        case("voided-detail-empty", "VoidedDetail vacío", obj("VoidedDetail")),
        case("voided-detail-full", "VoidedDetail completo", detail),
    ]
    for key, desc, instant, tz, cls in SUMMARY_DATES:
        for cls_name, prefix in (("Voided", "voided"), ("Reversion", "reversion")):
            cases.append(
                case(
                    f"{prefix}-date-{key}",
                    desc,
                    obj(cls_name, correlativo="9", fecComunicacion=dt(instant, tz, cls), company=company()),
                    calls,
                )
            )
    return cases


# --------------------------------------------------------------------------
# Retenciones y percepciones
# --------------------------------------------------------------------------

def payment(moneda: str, importe: str) -> dict:
    return obj("Payment", moneda=moneda, importe=dec(importe), fecha=dt("2024-04-10T09:00:00-05:00"))


def exchange() -> dict:
    return obj("Exchange", monedaRef="USD", monedaObj="PEN", factor=dec("3.712"), fecha=dt("2024-04-10T00:00:00-05:00"))


def retention_full(**over) -> dict:
    fields = dict(
        serie="R001",
        correlativo="10",
        fechaEmision=dt("2024-04-15T10:00:00-05:00"),
        company=company(),
        proveedor=client(),
        regimen="01",
        tasa=dec("3.00"),
        impRetenido=dec("30.00"),
        impPagado=dec("970.00"),
        observacion="Retención sintética",
        details=[
            obj(
                "RetentionDetail",
                tipoDoc="01",
                numDoc="F001-1",
                fechaEmision=dt("2024-04-01T10:00:00-05:00"),
                impTotal=dec("1000.00"),
                moneda="USD",
                pagos=[payment("USD", "500.00"), payment("USD", "500.00")],
                fechaRetencion=dt("2024-04-15T10:00:00-05:00"),
                impRetenido=dec("30.00"),
                impPagar=dec("970.00"),
                tipoCambio=exchange(),
            )
        ],
    )
    fields.update(over)
    return obj("Retention", **fields)


def perception_full(**over) -> dict:
    fields = dict(
        serie="P001",
        correlativo="11",
        fechaEmision=dt("2024-04-15T10:00:00-05:00"),
        company=company(),
        proveedor=client(),
        regimen="01",
        tasa=dec("2.00"),
        impPercibido=dec("20.00"),
        impCobrado=dec("1020.00"),
        observacion="Percepción sintética",
        details=[
            obj(
                "PerceptionDetail",
                tipoDoc="01",
                numDoc="F001-2",
                fechaEmision=dt("2024-04-01T10:00:00-05:00"),
                impTotal=dec("1000.00"),
                moneda="PEN",
                cobros=[payment("PEN", "1000.00")],
                fechaPercepcion=dt("2024-04-15T10:00:00-05:00"),
                impPercibido=dec("20.00"),
                impCobrar=dec("1020.00"),
                tipoCambio=exchange(),
            )
        ],
    )
    fields.update(over)
    return obj("Perception", **fields)


def retention_perception_cases() -> list[dict]:
    name = ["getName"]
    return [
        case("retention-empty", "Retention vacía", obj("Retention")),
        case("retention-full", "Retención completa: nombre con tipo 20", retention_full(), name),
        case("retention-name-no-company", "getName sin company falla", obj("Retention", serie="R001"), name),
        case("retention-name-null-parts", "Partes nulas del nombre", obj("Retention", company=obj("Company")), name),
        case("retention-detail-empty", "RetentionDetail vacío", obj("RetentionDetail")),
        case("payment-empty", "Payment vacío", obj("Payment")),
        case("exchange-empty", "Exchange vacío", obj("Exchange")),
        case("perception-empty", "Perception vacía", obj("Perception")),
        case("perception-full", "Percepción completa: nombre con tipo 40", perception_full(), name),
        case("perception-name-no-company", "getName sin company falla", obj("Perception", serie="P001"), name),
        case("perception-name-null-parts", "Partes nulas del nombre", obj("Perception", company=obj("Company")), name),
        case("perception-detail-empty", "PerceptionDetail vacío", obj("PerceptionDetail")),
    ]


# --------------------------------------------------------------------------
# Respuestas
# --------------------------------------------------------------------------

def cdr(code, **over) -> dict:
    fields = dict(
        id="F001-123",
        code=code,
        description="La Factura numero F001-123, ha sido aceptada",
        notes=["4252 - Nota sintética", "4287 - Otra nota"],
        reference="20000000001-01-F001-123",
    )
    fields.update(over)
    return obj("CdrResponse", **fields)


ACCEPTED_CODES = [
    # Cada valor se elige para que el resultado distinga la conversión exacta
    # de PHP `(int)`: un 0 (cadena no numérica) se considera aceptado.
    ("null", None),
    ("empty", ""),
    ("zero", "0"),
    ("zero-padded", "0000"),
    ("one", "1"),
    ("98", "98"),
    ("99", "99"),
    ("100", "100"),
    ("2000", "2000"),
    ("3999", "3999"),
    ("4000", "4000"),
    ("4252", "4252"),
    ("99999", "99999"),
    ("negative", "-1"),
    ("negative-zero", "-0"),
    ("plus-sign", "+1"),
    ("leading-spaces", "  1"),
    ("trailing-spaces", "1  "),
    ("leading-newline", "\n1"),
    ("leading-carriage-return", "\r1"),
    ("tab-prefix", "\t1"),
    ("vertical-tab-prefix", "\u000b1"),
    ("formfeed-prefix", "\u000c1"),
    ("nbsp-prefix", "\u00a01"),
    ("nul-prefix", "\u00001"),
    ("trailing-garbage", "1abc"),
    ("trailing-garbage-4000", "4000abc"),
    ("decimal-below", "3999.9"),
    ("decimal-rounds-up", "3999.99999999999999999"),
    ("decimal-above", "4000.5"),
    ("negative-fraction", "-0.9"),
    ("negative-decimal", "  -3999.99"),
    ("exponent", "4e3"),
    ("exponent-upper", "1E3"),
    ("exponent-fraction", "0.4e4"),
    ("exponent-negative", "4000e-1"),
    ("exponent-underflow", "1e-400"),
    ("exponent-incomplete", "1e"),
    ("exponent-sign-only", "1e+"),
    ("leading-dot", ".5"),
    ("leading-dot-exponent", "-.5e1"),
    ("trailing-dot", "1."),
    ("only-dot", "."),
    ("text", "abc"),
    ("hex", "0x1A"),
    ("underscore", "1_000"),
    ("int-max", "9223372036854775807"),
    ("int-overflow", "9223372036854775808"),
    ("int-min", "-9223372036854775808"),
    ("int-underflow", "-9223372036854775809"),
    ("float-overflow-positive", "1e19"),
    ("float-overflow-negative", "-1e19"),
    ("float-two-pow-64", "18446744073709551616"),
    ("float-near-max", "9.2233720368547748e18"),
    ("huge", "1e400"),
    ("negative-huge", "-1e400"),
    ("only-sign", "-"),
    ("double-sign", "+-1"),
    ("unicode-digits", "\u0661"),
    ("fullwidth-digits", "\uff11"),
    ("internal-space", "40 00"),
]


def response_cases() -> list[dict]:
    cases = [
        case("base-result-default", "BaseResult: success=false por defecto", obj("BaseResult")),
        case("base-result-null-success", "success admite null", obj("BaseResult", success=None)),
        case(
            "base-result-error",
            "BaseResult con error",
            obj("BaseResult", success=False, error=obj("Error", ["0111", "No tiene el perfil para enviar comprobantes electronicos"])),
        ),
        case(
            "bill-result-full",
            "BillResult con CDR y ZIP binario",
            obj("BillResult", success=True, cdrZip=raw(b"PK\x03\x04\x00\xff binario"), cdrResponse=cdr("0")),
        ),
        case("bill-result-empty", "BillResult vacío", obj("BillResult")),
        case("summary-result-full", "SummaryResult con ticket", obj("SummaryResult", success=True, ticket="1500000000001")),
        case("summary-result-empty", "SummaryResult vacío", obj("SummaryResult")),
        case(
            "status-result-full",
            "StatusResult con código 0 y CDR",
            obj("StatusResult", success=True, code="0", cdrZip=raw(b"cdr"), cdrResponse=cdr("0")),
        ),
        case(
            "status-result-processing",
            "StatusResult en proceso (98)",
            obj("StatusResult", success=False, code="98", error=obj("Error", ["98", "En proceso"])),
        ),
        case("status-result-empty", "StatusResult vacío", obj("StatusResult")),
        case(
            "status-cdr-result-full",
            "StatusCdrResult con código, mensaje y CDR",
            obj("StatusCdrResult", success=True, code="0004", message="La constancia existe", cdrZip=raw(b""), cdrResponse=cdr("0")),
        ),
        case("status-cdr-result-empty", "StatusCdrResult vacío", obj("StatusCdrResult")),
        case("error-default", "Error sin argumentos: código y mensaje vacíos", obj("Error")),
        case("error-code-only", "Error con un argumento: mensaje vacío", obj("Error", ["0100"])),
        case("error-nulls", "Error con null explícito", obj("Error", [None, None])),
        case("error-setters", "Error modificado con setters", obj("Error", code="2800", message="Mensaje")),
        case("cdr-empty", "CdrResponse vacío", obj("CdrResponse"), ["isAccepted"]),
        case("cdr-full", "CdrResponse completo", cdr("0"), ["isAccepted"]),
        case("cdr-empty-notes", "Notas vacías frente a null", cdr("0", notes=[]), ["isAccepted"]),
    ]
    for key, code in ACCEPTED_CODES:
        cases.append(case(f"cdr-accepted-{key}", f"isAccepted con código {code!r}", obj("CdrResponse", code=code), ["isAccepted"]))
    return cases


# --------------------------------------------------------------------------
# Servicios
# --------------------------------------------------------------------------

def services_cases() -> list[dict]:
    return [
        case("basic-token-null", "BasicToken con null", obj("BasicToken", [None, None])),
        case(
            "basic-token-full",
            "BasicToken con valor y expiración",
            obj("BasicToken", ["token-sintetico", dt("2024-01-01T12:00:00+00:00", "UTC")]),
        ),
        case(
            "basic-token-setters",
            "BasicToken modificado con setters",
            obj("BasicToken", [None, None], value="otro", expire=dt("2024-01-01T07:00:00-05:00", "America/Lima")),
        ),
        case("exception-default", "InvalidServiceResponseException sin argumentos", obj("InvalidServiceResponseException")),
        case(
            "exception-message-code",
            "InvalidServiceResponseException con mensaje y código",
            obj("InvalidServiceResponseException", ["Respuesta inválida", 500]),
        ),
        case(
            "exception-previous",
            "InvalidServiceResponseException conserva la causa",
            obj("InvalidServiceResponseException", ["Respuesta inválida", 0, obj("LogicException", ["causa", 7])]),
        ),
    ]


# --------------------------------------------------------------------------
# Importes, opcionales e inválidos
# --------------------------------------------------------------------------

AMOUNTS = [
    ("zero", "0"),
    ("zero-scale-2", "0.00"),
    ("negative-zero", "-0.00"),
    ("cent", "0.01"),
    ("negative", "-15.30"),
    ("three-decimals", "1234567.891"),
    ("binary-inexact", "0.1"),
    ("half-cent", "1.005"),
    ("thirteen-digits", "99999999999.99"),
    ("fifteen-significant", "1234567890123.45"),
    ("sixteen-significant", "12345678901234.56"),
    ("seventeen-significant", "123456789012345.67"),
    ("nineteen-significant", "12345678901234567.89"),
    ("many-decimals", "0.1234567890123456789"),
    ("tiny", "0.000001"),
    ("exponent-input", "1e3"),
    ("integer-trailing-zeros", "1000.000"),
    ("max-safe-integer", "9007199254740993"),
    ("decimal-max", "79228162514264337593543950335"),
    ("beyond-decimal-max", "79228162514264337593543950336"),
    ("large-exponent", "1e30"),
]


def amount_cases() -> list[dict]:
    cases = []
    for key, value in AMOUNTS:
        cases.append(case(f"amount-{key}", f"Importe {value}", obj("Charge", monto=dec(value), factor=dec(value))))
    cases.append(case("int-negative-bultos", "numBultos negativo", obj("Shipment", numBultos=-1)))
    cases.append(case("int-max-bultos", "numBultos máximo de 64 bits", obj("Shipment", numBultos=9223372036854775807)))
    cases.append(case("int-duracion-zero", "duracion cero", obj("DetailAttribute", duracion=0)))
    return cases


def optional_cases() -> list[dict]:
    return [
        case("optional-unset", "Campo no establecido", obj("Legend")),
        case("optional-null", "Campo establecido en null", obj("Legend", code=None, value=None)),
        case("optional-empty-string", "Cadena vacía", obj("Legend", code="", value="")),
        case("optional-zero-amount", "Importe cero", obj("Prepayment", total=dec("0"))),
        case("optional-zero-scale-amount", "Importe 0.00", obj("Prepayment", total=dec("0.00"))),
        case("optional-null-list", "Lista null", obj("Summary", details=None)),
        case("optional-empty-list", "Lista vacía", obj("Summary", details=[])),
        case("optional-false", "Booleano false frente a null", obj("Shipment", indTransbordo=False)),
        case("optional-null-object", "Objeto anidado null", obj("Invoice", company=None, client=None)),
    ]


def invalid_cases() -> list[dict]:
    return [
        case("invalid-string-into-float", "Texto en un importe", obj("Charge", monto="abc")),
        case("invalid-number-into-string", "Número en un campo de texto", obj("Legend", code=1000)),
        case("invalid-bool-into-string", "Booleano en un campo de texto", obj("Legend", value=True)),
        case("invalid-object-type", "Objeto vacío de otra clase", obj("Invoice", company=obj("Address"))),
        case(
            "invalid-object-type-with-fields",
            "Objeto de otra clase con datos",
            obj("Invoice", company=obj("Address", ubigueo="150101")),
        ),
        case("invalid-string-into-date-nested", "Texto en una fecha anidada", obj("Cuota", fechaPago="2024-01-01")),
        case("invalid-string-into-list-nested", "Texto en una lista de un modelo simple", obj("Summary", details="x")),
        case("invalid-array-into-string", "Lista en un campo de texto", obj("Legend", value=["a"])),
        case("invalid-string-into-int", "Texto en un entero", obj("Shipment", numBultos="3")),
        case("invalid-float-into-int", "Decimal en un entero", obj("Shipment", numBultos=dec("3.5"))),
        case("invalid-string-into-bool", "Texto en un booleano", obj("Shipment", indTransbordo="true")),
        case("invalid-string-into-date", "Texto en una fecha", obj("Invoice", fechaEmision="2024-01-01")),
        case("invalid-string-into-list", "Texto en una lista", obj("Invoice", details="x")),
        case("invalid-direction-missing-args", "Direction sin argumentos", obj("Direction")),
        case("invalid-token-missing-args", "BasicToken sin argumentos", obj("BasicToken")),
    ]


FAMILIES = {
    "identity": identity_cases,
    "sale": sale_cases,
    "despatch": despatch_cases,
    "summary": summary_cases,
    "voided": voided_cases,
    "retention-perception": retention_perception_cases,
    "response": response_cases,
    "services": services_cases,
    "amounts": amount_cases,
    "optional": optional_cases,
    "invalid": invalid_cases,
}


INT_CAST_EXTRA = [
    "1e19", "-1e19", "2e19", "-9223372036854775809", "9223372036854775807.5", "9.2233720368547748e18",
    "\r1", "1\n", " +1", "-.5e1", "0.4e4", "1E3", "  -3999.99", "4000.0e0", "1e-400", "-0.9", "", " ", "+", "+.", "-.e1",
    "00000000000000000000001", "-00000000000000000000001", "1" * 30, "-" + "1" * 30, "0.000000000000000000001e21",
]


def int_cast_inputs() -> list[str]:
    seen: list[str] = []
    for value in [code for _, code in ACCEPTED_CODES if code is not None] + INT_CAST_EXTRA:
        if value not in seen:
            seen.append(value)
    return seen


def main() -> None:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "tests/reference/core/cases"
    out_dir.mkdir(parents=True, exist_ok=True)
    seen: set[str] = set()
    for family, build in FAMILIES.items():
        cases = build()
        for c in cases:
            if c["id"] in seen:
                raise SystemExit(f"id duplicado: {c['id']}")
            seen.add(c["id"])
        text = json.dumps({"family": family, "cases": cases}, ensure_ascii=False, indent=2) + "\n"
        (out_dir / f"{family}.json").write_text(text, encoding="utf-8")
    int_cast_dir = out_dir.parent / "int-cast"
    int_cast_dir.mkdir(parents=True, exist_ok=True)
    text = json.dumps({"inputs": int_cast_inputs()}, ensure_ascii=False, indent=2) + "\n"
    (int_cast_dir / "inputs.json").write_text(text, encoding="utf-8")


if __name__ == "__main__":
    main()
