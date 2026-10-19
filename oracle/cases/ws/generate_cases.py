#!/usr/bin/env python3
"""Casos sintéticos de ws v5.3.0; solo bytes locales, nunca credenciales reales."""

import json
import sys

CAC = "urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2"
CBC = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2"
CDR = (
    f'<ApplicationResponse xmlns="urn:oasis:names:specification:ubl:schema:xsd:ApplicationResponse-2" '
    f'xmlns:cac="{CAC}" xmlns:cbc="{CBC}">'
    '<cbc:Note>Observación ficticia</cbc:Note>'
    '<cac:DocumentResponse><cac:Response><cbc:ReferenceID>F001-1</cbc:ReferenceID>'
    '<cbc:ResponseCode>0</cbc:ResponseCode><cbc:Description>Aceptada</cbc:Description>'
    '</cac:Response><cac:DocumentReference><cbc:DocumentDescription>referencia-local</cbc:DocumentDescription>'
    '</cac:DocumentReference></cac:DocumentResponse></ApplicationResponse>'
)


def invoice(version="2.1", root="Invoice", doc_id="F001-1", kind="01"):
    supplier = (
        '<cac:AccountingSupplierParty><cbc:CustomerAssignedAccountID>20000000001</cbc:CustomerAssignedAccountID></cac:AccountingSupplierParty>'
        if version == "2.0"
        else '<cac:AccountingSupplierParty><cac:Party><cac:PartyIdentification><cbc:ID>20000000001</cbc:ID></cac:PartyIdentification></cac:Party></cac:AccountingSupplierParty>'
    )
    return (
        f'<{root} xmlns="urn:oasis:names:specification:ubl:schema:xsd:{root}-2" '
        f'xmlns:cac="{CAC}" xmlns:cbc="{CBC}">'
        f'<cbc:UBLVersionID>{version}</cbc:UBLVersionID><cbc:ID>{doc_id}</cbc:ID>'
        f'<cbc:InvoiceTypeCode>{kind}</cbc:InvoiceTypeCode>{supplier}</{root}>'
    )


def invoice_lines(count):
    """Factura con líneas distintas; con 200 líneas (~55 KiB) memLevel 8 y 9 de zlib difieren."""
    lines = "".join(
        f'<cac:InvoiceLine><cbc:ID>{n}</cbc:ID>'
        f'<cbc:InvoicedQuantity unitCode="NIU">{n * 7 % 97}</cbc:InvoicedQuantity>'
        f'<cbc:LineExtensionAmount currencyID="PEN">{n * 1301 % 10007}.{n * 37 % 100:02d}</cbc:LineExtensionAmount>'
        f'<cac:Item><cbc:Description>PRODUCTO SINTETICO {n * 7919 % 100003}</cbc:Description></cac:Item>'
        f'</cac:InvoiceLine>'
        for n in range(1, count + 1)
    )
    return invoice().replace("</Invoice>", lines + "</Invoice>")


cases = [
    {"id": "endpoints", "kind": "endpoints"},
    {"id": "zip-one", "kind": "zip", "name": "20000000001-01-F001-1.xml", "content": invoice()},
    {"id": "zip-empty", "kind": "zip", "name": "vacio.xml", "content": ""},
    {"id": "zip-traversal", "kind": "zip", "name": "../escape.xml", "content": "<x/>"},
    {"id": "zip-utf8-name", "kind": "zip", "name": "20000000001-01-F001-Ñ.xml", "content": "<x>á</x>"},
    {"id": "zip-large", "kind": "zip", "name": "20000000001-01-F001-2.xml", "content": invoice() * 40},
    {"id": "zip-varied-55k", "kind": "zip", "name": "20000000001-01-F001-3.xml", "content": invoice_lines(200)},
    {"id": "cdr-ok", "kind": "cdr", "xml": CDR},
    {"id": "cdr-no-notes", "kind": "cdr", "xml": CDR.replace("<cbc:Note>Observación ficticia</cbc:Note>", "")},
    {"id": "cdr-empty", "kind": "cdr", "xml": ""},
    {"id": "cdr-invalid", "kind": "cdr", "xml": "<ApplicationResponse"},
    {"id": "cdr-xxe", "kind": "cdr", "xml": '<!DOCTYPE x [<!ENTITY ext SYSTEM "file:///ruta-inexistente-sintetica/xxe.txt">]><x>&ext;</x>'},
    {"id": "name-invoice-21", "kind": "name", "xml": invoice()},
    {"id": "name-invoice-20", "kind": "name", "xml": invoice("2.0")},
    {"id": "name-credit", "kind": "name", "xml": invoice(root="CreditNote", kind="07")},
    {"id": "name-ubl-unknown", "kind": "name", "xml": invoice("3.0")},
    {"id": "type-invoice", "kind": "type", "xml": invoice()},
    {"id": "type-credit", "kind": "type", "xml": invoice(root="CreditNote")},
    {"id": "type-voided-ra", "kind": "type", "xml": invoice(root="VoidedDocuments", doc_id="RA-20240101-1")},
    {"id": "type-voided-other", "kind": "type", "xml": invoice(root="VoidedDocuments", doc_id="RR-20240101-1")},
    {"id": "type-unknown", "kind": "type", "xml": "<Other/>"},
    {"id": "bill-cdr", "kind": "bill", "response": "cdr", "name": "20000000001-01-F001-1", "content": invoice()},
    {"id": "bill-no-cdr", "kind": "bill", "response": "empty", "name": "20000000001-01-F001-1", "content": invoice()},
    {"id": "bill-fault", "kind": "bill", "response": "fault", "name": "20000000001-01-F001-1", "content": invoice()},
    {"id": "summary-ticket", "kind": "summary", "response": "ticket", "name": "20000000001-RC-20240101-1", "content": invoice(root="SummaryDocuments", doc_id="RC-20240101-1")},
    {"id": "summary-fault", "kind": "summary", "response": "fault", "name": "20000000001-RC-20240101-1", "content": invoice(root="SummaryDocuments", doc_id="RC-20240101-1")},
    {"id": "ext-pending", "kind": "ext", "response": "pending", "ticket": "TICKET-FICTICIO"},
    {"id": "ext-missing", "kind": "ext", "response": "missing", "ticket": "TICKET-FICTICIO"},
    {"id": "ext-no-cdr", "kind": "ext", "response": "empty", "ticket": "TICKET-FICTICIO"},
    {"id": "ext-cdr", "kind": "ext", "response": "cdr", "ticket": "TICKET-FICTICIO"},
    {"id": "consult-status", "kind": "consult", "response": "status", "ruc": "20000000001", "tipo": "01", "serie": "F001", "numero": 1},
    {"id": "consult-cdr", "kind": "consult", "response": "cdr", "ruc": "20000000001", "tipo": "01", "serie": "F001", "numero": 1},
    {"id": "soap-send-bill", "kind": "soap", "operation": "sendBill", "name": "20000000001-01-F001-1", "content": invoice()},
    {"id": "soap-send-summary", "kind": "soap", "operation": "sendSummary", "name": "20000000001-RC-20240101-1", "content": invoice(root="SummaryDocuments", doc_id="RC-20240101-1")},
    {"id": "soap-get-status", "kind": "soap", "operation": "getStatus", "ticket": "TICKET-FICTICIO"},
    {"id": "soap-get-status-cdr", "kind": "soap", "operation": "getStatusCdr", "ruc": "20000000001", "tipo": "01", "serie": "F001", "numero": 1},
    {"id": "soap-fault", "kind": "soap", "operation": "sendBill", "name": "20000000001-01-F001-1", "content": invoice()},
    {"id": "gre-send", "kind": "gre-send", "name": "20000000001-09-T001-1", "content": invoice(root="DespatchAdvice", doc_id="T001-1", kind="09")},
    {"id": "gre-status-98", "kind": "gre-status", "response": "98", "ticket": "TICKET-FICTICIO"},
    {"id": "gre-status-99", "kind": "gre-status", "response": "99", "ticket": "TICKET-FICTICIO"},
    {"id": "gre-status-0", "kind": "gre-status", "response": "0", "ticket": "TICKET-FICTICIO"},
    {"id": "gre-status-422", "kind": "gre-status", "response": "422", "ticket": "TICKET-FICTICIO"},
    {"id": "factory-new", "kind": "factory", "response": "new"},
    {"id": "factory-cache", "kind": "factory", "response": "cache"},
    {"id": "factory-expired", "kind": "factory", "response": "expired"},
    {"id": "factory-empty", "kind": "factory", "response": "empty"},
]

target = sys.argv[1] if len(sys.argv) > 1 else "tests/reference/ws/cases.json"
with open(target, "w", encoding="utf-8") as file:
    json.dump({"source": "thegreenter/greenter v5.3.0 f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f", "cases": cases}, file, ensure_ascii=False, indent=2)
    file.write("\n")
