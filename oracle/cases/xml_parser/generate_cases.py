#!/usr/bin/env python3
"""Casos sintéticos de paridad de greenter/xml-parser v5.3.0.

Todos los XML se construyen aquí con datos ficticios (RUC 20000000001 y
20000000002, DNI 00000001, nombres «SINTETICA»). Ningún XML upstream de
packages/xml-parser/tests/Resources se copia: contienen datos de terceros.
Además se reutilizan, por ruta, XML sintéticos ya versionados que generó
Greenter PHP para cpeforge-xml (tests/reference/xml/expected).

Uso: python3 oracle/cases/xml_parser/generate_cases.py [salida.json]
"""

from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

NS = {
    "inv": "urn:oasis:names:specification:ubl:schema:xsd:Invoice-2",
    "cn": "urn:oasis:names:specification:ubl:schema:xsd:CreditNote-2",
    "dn": "urn:oasis:names:specification:ubl:schema:xsd:DebitNote-2",
    "da": "urn:oasis:names:specification:ubl:schema:xsd:DespatchAdvice-2",
    "sd": "urn:sunat:names:specification:ubl:peru:schema:xsd:SummaryDocuments-1",
    "vd": "urn:sunat:names:specification:ubl:peru:schema:xsd:VoidedDocuments-1",
    "ret": "urn:sunat:names:specification:ubl:peru:schema:xsd:Retention-1",
    "per": "urn:sunat:names:specification:ubl:peru:schema:xsd:Perception-1",
}
COMMON = (
    'xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2" '
    'xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2" '
    'xmlns:ext="urn:oasis:names:specification:ubl:schema:xsd:CommonExtensionComponents-2" '
    'xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1" '
    'xmlns:ds="http://www.w3.org/2000/09/xmldsig#"'
)
DECL = '<?xml version="1.0" encoding="UTF-8"?>\n'
UBL_XML = "tests/reference/xml/expected"


def root(name: str, ns_key: str, body: str, prefix: str | None = None, extra_ns: str = COMMON) -> str:
    tag = f"{prefix}:{name}" if prefix else name
    xmlns = f'xmlns:{prefix}="{NS[ns_key]}"' if prefix else f'xmlns="{NS[ns_key]}"'
    return f"{DECL}<{tag} {xmlns} {extra_ns}>{body}</{tag}>"


def party(tag: str, doc: str, kind: str, name: str, comercial: str | None = "COMERCIAL SINTETICO", address: bool = True) -> str:
    addr = (
        "<cac:PostalAddress><cbc:ID>150101</cbc:ID><cbc:StreetName>AV. SINTETICA 123</cbc:StreetName>"
        "<cbc:CityName>LIMA</cbc:CityName><cbc:CountrySubentity>LIMA</cbc:CountrySubentity>"
        "<cbc:District>LIMA</cbc:District><cac:Country><cbc:IdentificationCode>PE</cbc:IdentificationCode></cac:Country></cac:PostalAddress>"
        if address else ""
    )
    pname = f"<cac:PartyName><cbc:Name>{comercial}</cbc:Name></cac:PartyName>" if comercial else ""
    return (
        f"<cac:{tag}><cbc:CustomerAssignedAccountID>{doc}</cbc:CustomerAssignedAccountID>"
        f"<cbc:AdditionalAccountID>{kind}</cbc:AdditionalAccountID><cac:Party>{pname}{addr}"
        f"<cac:PartyLegalEntity><cbc:RegistrationName>{name}</cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:{tag}>"
    )


def additional(totals: str, legends: str = "", transaction: str = "0101") -> str:
    return (
        "<ext:UBLExtensions><ext:UBLExtension><ext:ExtensionContent><sac:AdditionalInformation>"
        f"{totals}{legends}<sac:SUNATTransaction><cbc:ID>{transaction}</cbc:ID></sac:SUNATTransaction>"
        "</sac:AdditionalInformation></ext:ExtensionContent></ext:UBLExtension>"
        "<ext:UBLExtension><ext:ExtensionContent></ext:ExtensionContent></ext:UBLExtension></ext:UBLExtensions>"
    )


def amt(ident: str, value: str, extra: str = "", scheme: str = "") -> str:
    sch = f' schemeID="{scheme}"' if scheme else ""
    return f'<sac:AdditionalMonetaryTotal><cbc:ID{sch}>{ident}</cbc:ID><cbc:PayableAmount currencyID="PEN">{value}</cbc:PayableAmount>{extra}</sac:AdditionalMonetaryTotal>'


TOTALS = (
    amt("1001", "100.00") + amt("1002", "20.50") + amt("1003", "3.25") + amt("1004", "0.10")
    + amt("2001", "2.00", '<sac:ReferenceAmount currencyID="PEN">100.00</sac:ReferenceAmount><sac:TotalAmount currencyID="PEN">120.00</sac:TotalAmount>', scheme="01")
    + amt("2003", "12.00", "<cbc:Percent>10</cbc:Percent><sac:ReferenceAmount currencyID=\"PEN\">118.00</sac:ReferenceAmount>")
    + amt("2005", "5.00") + amt(" 9999 ", "1.00")
)
LEGENDS = (
    "<sac:AdditionalProperty><cbc:ID>1000</cbc:ID><cbc:Value>CIENTO DIECIOCHO CON 00/100 SOLES</cbc:Value></sac:AdditionalProperty>"
    "<sac:AdditionalProperty><cbc:ID>2006</cbc:ID><cbc:Value>Operación sujeta a detracción &amp; «ñ»</cbc:Value></sac:AdditionalProperty>"
)


def tax(name: str, amount: str, extra_cat: str = "") -> str:
    return (
        f'<cac:TaxTotal><cbc:TaxAmount currencyID="PEN">{amount}</cbc:TaxAmount><cac:TaxSubtotal>'
        f'<cbc:TaxAmount currencyID="PEN">{amount}</cbc:TaxAmount><cac:TaxCategory>{extra_cat}'
        f"<cac:TaxScheme><cbc:ID>1000</cbc:ID><cbc:Name>{name}</cbc:Name><cbc:TaxTypeCode>VAT</cbc:TaxTypeCode></cac:TaxScheme>"
        "</cac:TaxCategory></cac:TaxSubtotal></cac:TaxTotal>"
    )


def line(tag: str, qty_tag: str, n: int, qty: str = "2", unit: str = "NIU") -> str:
    return (
        f'<cac:{tag}><cbc:ID>{n}</cbc:ID><cbc:{qty_tag} unitCode="{unit}">{qty}</cbc:{qty_tag}>'
        '<cbc:LineExtensionAmount currencyID="PEN">50.00</cbc:LineExtensionAmount>'
        '<cac:PricingReference><cac:AlternativeConditionPrice><cbc:PriceAmount currencyID="PEN">29.50</cbc:PriceAmount><cbc:PriceTypeCode> 01 </cbc:PriceTypeCode></cac:AlternativeConditionPrice></cac:PricingReference>'
        '<cac:PricingReference><cac:AlternativeConditionPrice><cbc:PriceAmount currencyID="PEN">3.00</cbc:PriceAmount><cbc:PriceTypeCode>02</cbc:PriceTypeCode></cac:AlternativeConditionPrice></cac:PricingReference>'
        '<cac:AllowanceCharge><cbc:ChargeIndicator> false </cbc:ChargeIndicator><cbc:Amount currencyID="PEN">1.50</cbc:Amount></cac:AllowanceCharge>'
        '<cac:AllowanceCharge><cbc:ChargeIndicator>true</cbc:ChargeIndicator><cbc:Amount currencyID="PEN">9.99</cbc:Amount></cac:AllowanceCharge>'
        + tax("IGV", "9.00", "<cbc:TaxExemptionReasonCode>10</cbc:TaxExemptionReasonCode>")
        + tax("ISC", "1.00", "<cbc:TierRange>01</cbc:TierRange>")
        + f"<cac:Item><cbc:Description><![CDATA[PRODUCTO <SINTETICO> {n}]]></cbc:Description>"
        "<cac:SellersItemIdentification><cbc:ID>P001</cbc:ID></cac:SellersItemIdentification>"
        "<cac:CommodityClassification><cbc:ItemClassificationCode>10191509</cbc:ItemClassificationCode></cac:CommodityClassification></cac:Item>"
        f'<cac:Price><cbc:PriceAmount currencyID="PEN">25.00</cbc:PriceAmount></cac:Price></cac:{tag}>'
    )


def invoice_body(id_="F001-123", date="2024-01-15", lines=2, extensions=True, more="") -> str:
    ext = additional(TOTALS, LEGENDS) if extensions else ""
    return (
        f"{ext}<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:ID>{id_}</cbc:ID><cbc:IssueDate>{date}</cbc:IssueDate>"
        "<cbc:InvoiceTypeCode>01</cbc:InvoiceTypeCode><cbc:DocumentCurrencyCode>PEN</cbc:DocumentCurrencyCode>"
        "<cac:OrderReference><cbc:ID>OC-77</cbc:ID></cac:OrderReference>"
        "<cac:DespatchDocumentReference><cbc:ID>T001-1</cbc:ID><cbc:DocumentTypeCode>09</cbc:DocumentTypeCode></cac:DespatchDocumentReference>"
        "<cac:DespatchDocumentReference><cbc:ID>T001-2</cbc:ID><cbc:DocumentTypeCode>31</cbc:DocumentTypeCode></cac:DespatchDocumentReference>"
        + party("AccountingSupplierParty", "20000000001", "6", "EMPRESA SINTETICA S.A.C.")
        + party("AccountingCustomerParty", "20000000002", "6", "CLIENTE SINTETICO S.A.", comercial=None)
        + "<cac:PaymentMeans><cbc:PaymentMeansCode>001</cbc:PaymentMeansCode><cbc:PaymentDueDate>2024-02-15</cbc:PaymentDueDate></cac:PaymentMeans>"
        + '<cac:PrepaidPayment><cbc:ID schemeID="02">F001-10</cbc:ID><cbc:PaidAmount currencyID="PEN">10.00</cbc:PaidAmount></cac:PrepaidPayment>'
        + '<cac:PrepaidPayment><cbc:ID schemeID="03">B001-11</cbc:ID><cbc:PaidAmount currencyID="PEN">5.50</cbc:PaidAmount></cac:PrepaidPayment>'
        + tax("IGV", "18.00") + tax("ISC", "2.00") + tax("OTROS", "0.50") + tax("IVAP", "7.00")
        + '<cac:LegalMonetaryTotal><cbc:AllowanceTotalAmount currencyID="PEN">1.00</cbc:AllowanceTotalAmount>'
        '<cbc:ChargeTotalAmount currencyID="PEN">0.75</cbc:ChargeTotalAmount><cbc:PrepaidAmount currencyID="PEN">15.50</cbc:PrepaidAmount>'
        '<cbc:PayableAmount currencyID="PEN">118.00</cbc:PayableAmount></cac:LegalMonetaryTotal>'
        + more
        + "".join(line("InvoiceLine", "InvoicedQuantity", i + 1) for i in range(lines))
    )


def note_body(credit=True, id_="FC01-5", more="", lines=1, extensions=True) -> str:
    total_tag = "LegalMonetaryTotal" if credit else "RequestedMonetaryTotal"
    ext = additional(amt("1001", "40.00") + amt("2001", "1.00", "<sac:ReferenceAmount>40</sac:ReferenceAmount><sac:TotalAmount>41</sac:TotalAmount>", "02"), LEGENDS) if extensions else ""
    return (
        f"{ext}<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:ID>{id_}</cbc:ID><cbc:IssueDate>2024-03-01</cbc:IssueDate>"
        "<cbc:DocumentCurrencyCode>USD</cbc:DocumentCurrencyCode>"
        "<cac:DiscrepancyResponse><cbc:ReferenceID>F001-123</cbc:ReferenceID><cbc:ResponseCode>01</cbc:ResponseCode><cbc:Description>ANULACION SINTETICA</cbc:Description></cac:DiscrepancyResponse>"
        "<cac:BillingReference><cac:InvoiceDocumentReference><cbc:ID>F001-123</cbc:ID><cbc:DocumentTypeCode>01</cbc:DocumentTypeCode></cac:InvoiceDocumentReference></cac:BillingReference>"
        "<cac:DespatchDocumentReference><cbc:ID>T001-9</cbc:ID><cbc:DocumentTypeCode>09</cbc:DocumentTypeCode></cac:DespatchDocumentReference>"
        + party("AccountingSupplierParty", "20000000001", "6", "EMPRESA SINTETICA S.A.C.")
        + party("AccountingCustomerParty", "00000001", "1", "PERSONA SINTETICA", comercial=None, address=False)
        + tax(" IGV ", "7.20") + tax("OTROS", "0.30")
        + f'<cac:{total_tag}><cbc:ChargeTotalAmount currencyID="USD">0.30</cbc:ChargeTotalAmount><cbc:PayableAmount currencyID="USD">47.50</cbc:PayableAmount></cac:{total_tag}>'
        + more
        + "".join(line("CreditNoteLine" if credit else "DebitNoteLine", "CreditedQuantity" if credit else "DebitedQuantity", i + 1) for i in range(lines))
    )


def despatch_body(id_="T001-00000045", third=True, shipment=True, more="", lines=2) -> str:
    third_xml = (
        '<cac:SellerSupplierParty><cbc:CustomerAssignedAccountID schemeID="6">20000000003</cbc:CustomerAssignedAccountID>'
        "<cac:Party><cac:PartyLegalEntity><cbc:RegistrationName>TERCERO SINTETICO</cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:SellerSupplierParty>"
        if third else ""
    )
    ship = (
        "<cac:Shipment><cbc:ID>1</cbc:ID><cbc:HandlingCode>01</cbc:HandlingCode><cbc:Information>VENTA</cbc:Information>"
        '<cbc:GrossWeightMeasure unitCode="KGM">12.500</cbc:GrossWeightMeasure><cbc:TotalTransportHandlingUnitQuantity>3</cbc:TotalTransportHandlingUnitQuantity>'
        "<cbc:SplitConsignmentIndicator>true</cbc:SplitConsignmentIndicator>"
        "<cac:ShipmentStage><cbc:TransportModeCode>01</cbc:TransportModeCode><cac:TransitPeriod><cbc:StartDate>2024-04-02</cbc:StartDate></cac:TransitPeriod>"
        '<cac:CarrierParty><cac:PartyIdentification><cbc:ID schemeID="6">20000000004</cbc:ID></cac:PartyIdentification><cac:PartyName><cbc:Name>TRANSPORTES SINTETICOS</cbc:Name></cac:PartyName></cac:CarrierParty>'
        "<cac:TransportMeans><cac:RoadTransport><cbc:LicensePlateID>ABC-123</cbc:LicensePlateID></cac:RoadTransport></cac:TransportMeans>"
        '<cac:DriverPerson><cbc:ID schemeID="1">00000002</cbc:ID></cac:DriverPerson></cac:ShipmentStage>'
        "<cac:Delivery><cac:DeliveryAddress><cbc:ID>150203</cbc:ID><cbc:StreetName>JR. LLEGADA 1</cbc:StreetName></cac:DeliveryAddress></cac:Delivery>"
        "<cac:TransportHandlingUnit><cbc:ID>CONT-1</cbc:ID></cac:TransportHandlingUnit>"
        "<cac:OriginAddress><cbc:ID>150101</cbc:ID><cbc:StreetName>AV. PARTIDA 9</cbc:StreetName></cac:OriginAddress>"
        "<cac:FirstArrivalPortLocation><cbc:ID>PAI</cbc:ID></cac:FirstArrivalPortLocation></cac:Shipment>"
        if shipment else ""
    )
    lines_xml = "".join(
        f'<cac:DespatchLine><cbc:ID>{i+1}</cbc:ID><cbc:DeliveredQuantity unitCode="NIU">{i+1}.5</cbc:DeliveredQuantity>'
        f"<cac:OrderLineReference><cbc:LineID>{i+1}</cbc:LineID></cac:OrderLineReference><cac:Item><cbc:Name>BIEN SINTETICO {i+1}</cbc:Name>"
        "<cac:SellersItemIdentification><cbc:ID>B01</cbc:ID></cac:SellersItemIdentification>"
        "<cac:CommodityClassification><cbc:ItemClassificationCode>43211706</cbc:ItemClassificationCode></cac:CommodityClassification></cac:Item></cac:DespatchLine>"
        for i in range(lines)
    )
    return (
        f"<cbc:UBLVersionID>2.1</cbc:UBLVersionID><cbc:ID>{id_}</cbc:ID><cbc:IssueDate>2024-04-01</cbc:IssueDate>"
        "<cbc:DespatchAdviceTypeCode>09</cbc:DespatchAdviceTypeCode><cbc:Note>OBSERVACION SINTETICA</cbc:Note>"
        "<cac:OrderReference><cbc:ID>T001-1</cbc:ID><cbc:OrderTypeCode>09</cbc:OrderTypeCode></cac:OrderReference>"
        "<cac:AdditionalDocumentReference><cbc:ID>F001-1</cbc:ID><cbc:DocumentTypeCode>01</cbc:DocumentTypeCode><cbc:OrderTypeCode>01</cbc:OrderTypeCode></cac:AdditionalDocumentReference>"
        "<cac:DespatchSupplierParty><cbc:CustomerAssignedAccountID schemeID=\"6\">20000000001</cbc:CustomerAssignedAccountID>"
        "<cac:Party><cac:PartyLegalEntity><cbc:RegistrationName>EMPRESA SINTETICA S.A.C.</cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:DespatchSupplierParty>"
        '<cac:DeliveryCustomerParty><cbc:CustomerAssignedAccountID schemeID="6">20000000002</cbc:CustomerAssignedAccountID>'
        "<cac:Party><cac:PartyLegalEntity><cbc:RegistrationName>DESTINATARIO SINTETICO</cbc:RegistrationName></cac:PartyLegalEntity></cac:Party></cac:DeliveryCustomerParty>"
        + third_xml + ship + more + lines_xml
    )


def receipt_xml(id_="E001-27", more="") -> str:
    body = (
        f"<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:ID>{id_}</cbc:ID><cbc:IssueDate>2024-05-10</cbc:IssueDate>"
        "<cbc:Note>MIL CON 00/100 SOLES</cbc:Note>"
        "<cac:AccountingSupplierParty><cbc:CustomerAssignedAccountID>10000000001</cbc:CustomerAssignedAccountID><cbc:AdditionalAccountID>6</cbc:AdditionalAccountID>"
        "<cac:Party><cac:PartyName><cbc:Name>PROFESIONAL SINTETICO</cbc:Name></cac:PartyName><cac:PostalAddress><cbc:StreetName>CALLE HONORARIOS 5</cbc:StreetName></cac:PostalAddress></cac:Party></cac:AccountingSupplierParty>"
        "<cac:AccountingCustomerParty><cbc:CustomerAssignedAccountID>20000000002</cbc:CustomerAssignedAccountID><cbc:AdditionalAccountID>6</cbc:AdditionalAccountID>"
        "<cac:Party><cac:PartyName><cbc:Name>CLIENTE SINTETICO S.A.</cbc:Name></cac:PartyName></cac:Party></cac:AccountingCustomerParty>"
        '<cac:TaxTotal><cbc:TaxAmount currencyID="PEN">80.00</cbc:TaxAmount><cac:TaxSubtotal><cbc:TaxableAmount currencyID="PEN">1000.00</cbc:TaxableAmount>'
        '<cbc:TaxAmount currencyID="PEN">80.00</cbc:TaxAmount></cac:TaxSubtotal></cac:TaxTotal>'
        '<cac:LegalMonetaryTotal><cbc:TaxExclusiveAmount currencyID="PEN">80.00</cbc:TaxExclusiveAmount><cbc:PayableAmount currencyID="PEN">920.00</cbc:PayableAmount></cac:LegalMonetaryTotal>'
        '<cac:InvoiceLine><cbc:ID>1</cbc:ID><cac:TaxTotal><cac:TaxSubtotal><cbc:Percent>8</cbc:Percent></cac:TaxSubtotal></cac:TaxTotal>'
        "<cac:Item><cbc:Description>SERVICIO PROFESIONAL SINTETICO</cbc:Description></cac:Item></cac:InvoiceLine>" + more
    )
    return root("Invoice", "inv", body)


def summary_xml(id_="RC-20240601-1", more="") -> str:
    det = (
        "<sac:SummaryDocumentsLine><cbc:LineID>1</cbc:LineID><cbc:DocumentTypeCode>03</cbc:DocumentTypeCode><cbc:ID>B001-1</cbc:ID>"
        "<cac:AccountingCustomerParty><cbc:CustomerAssignedAccountID> 00000001 </cbc:CustomerAssignedAccountID><cbc:AdditionalAccountID> 1 </cbc:AdditionalAccountID></cac:AccountingCustomerParty>"
        "<cac:BillingReference><cac:InvoiceDocumentReference><cbc:ID> B001-0 </cbc:ID><cbc:DocumentTypeCode> 03 </cbc:DocumentTypeCode></cac:InvoiceDocumentReference></cac:BillingReference>"
        "<sac:SUNATPerceptionSummaryDocumentReference><sac:SUNATPerceptionSystemCode> 01 </sac:SUNATPerceptionSystemCode><sac:SUNATPerceptionPercent>2.00</sac:SUNATPerceptionPercent>"
        '<sac:TotalInvoiceAmount currencyID="PEN">2.00</sac:TotalInvoiceAmount><sac:SUNATTotalCashed currencyID="PEN">102.00</sac:SUNATTotalCashed><sac:TaxableAmount currencyID="PEN">100.00</sac:TaxableAmount></sac:SUNATPerceptionSummaryDocumentReference>'
        "<cac:Status><cbc:ConditionCode> 1 </cbc:ConditionCode></cac:Status>"
        '<sac:TotalAmount currencyID="PEN">118.00</sac:TotalAmount>'
        '<sac:BillingPayment><cbc:PaidAmount currencyID="PEN">100.00</cbc:PaidAmount><cbc:InstructionID>01</cbc:InstructionID></sac:BillingPayment>'
        '<sac:BillingPayment><cbc:PaidAmount currencyID="PEN">5.00</cbc:PaidAmount><cbc:InstructionID>02</cbc:InstructionID></sac:BillingPayment>'
        '<sac:BillingPayment><cbc:PaidAmount currencyID="PEN">4.00</cbc:PaidAmount><cbc:InstructionID>03</cbc:InstructionID></sac:BillingPayment>'
        '<sac:BillingPayment><cbc:PaidAmount currencyID="PEN">3.00</cbc:PaidAmount><cbc:InstructionID> 05 </cbc:InstructionID></sac:BillingPayment>'
        '<cac:AllowanceCharge><cbc:ChargeIndicator>true</cbc:ChargeIndicator><cbc:Amount currencyID="PEN">0.50</cbc:Amount></cac:AllowanceCharge>'
        + tax("IGV", "18.00") + tax("ISC", "1.00") + tax("OTROS", "0.20")
        + "</sac:SummaryDocumentsLine>"
        "<sac:SummaryDocumentsLine><cbc:LineID>2</cbc:LineID><cbc:DocumentTypeCode>07</cbc:DocumentTypeCode><cbc:ID>BC01-2</cbc:ID>"
        "<cac:Status><cbc:ConditionCode>3</cbc:ConditionCode></cac:Status><sac:TotalAmount>10</sac:TotalAmount></sac:SummaryDocumentsLine>"
    )
    body = (
        f"<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:CustomizationID>1.1</cbc:CustomizationID><cbc:ID>{id_}</cbc:ID>"
        "<cbc:ReferenceDate>2024-05-31</cbc:ReferenceDate><cbc:IssueDate>2024-06-01</cbc:IssueDate>"
        + party("AccountingSupplierParty", "20000000001", "6", "EMPRESA SINTETICA S.A.C.", address=False) + det + more
    )
    return root("SummaryDocuments", "sd", body)


def voided_xml(id_="RA-20240601-7", more="") -> str:
    det = "".join(
        f"<sac:VoidedDocumentsLine><cbc:LineID>{i}</cbc:LineID><cbc:DocumentTypeCode>01</cbc:DocumentTypeCode>"
        f"<sac:DocumentSerialID>F001</sac:DocumentSerialID><sac:DocumentNumberID>{100 + i}</sac:DocumentNumberID>"
        f"<sac:VoidReasonDescription>ERROR SINTETICO {i}</sac:VoidReasonDescription></sac:VoidedDocumentsLine>"
        for i in (1, 2)
    )
    body = (
        f"<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:ID>{id_}</cbc:ID><cbc:ReferenceDate>2024-05-30</cbc:ReferenceDate>"
        "<cbc:IssueDate>2024-06-01</cbc:IssueDate>"
        + party("AccountingSupplierParty", "20000000001", "6", "EMPRESA SINTETICA S.A.C.", address=False) + det + more
    )
    return root("VoidedDocuments", "vd", body)


def cp_xml(kind: str, id_: str, info=True, exchange=True, more="", details=1) -> str:
    """Retención (kind='Retention') o percepción (kind='Perception')."""
    pre = "SUNAT" + kind
    paid = "Paid" if kind == "Retention" else "Cashed"
    amount = "Amount"
    info_xml = (
        f"<sac:{pre}Information><sac:{pre}{amount} currencyID=\"PEN\">3.00</sac:{pre}{amount}>"
        f"<sac:{pre}Date>2024-07-02</sac:{pre}Date><sac:SUNATNetTotal{paid} currencyID=\"PEN\">97.00</sac:SUNATNetTotal{paid}>"
        + ("<cac:ExchangeRate><cbc:SourceCurrencyCode>USD</cbc:SourceCurrencyCode><cbc:TargetCurrencyCode>PEN</cbc:TargetCurrencyCode>"
           "<cbc:CalculationRate>3.785</cbc:CalculationRate><cbc:Date>2024-07-01</cbc:Date></cac:ExchangeRate>" if exchange else "")
        + f"</sac:{pre}Information>"
        if info else ""
    )
    det = "".join(
        f"<sac:{pre}DocumentReference><cbc:ID schemeID=\"01\">F001-{i+1}</cbc:ID><cbc:IssueDate>2024-06-20</cbc:IssueDate>"
        f'<cbc:TotalInvoiceAmount currencyID="USD">100.00</cbc:TotalInvoiceAmount>'
        f'<cac:Payment><cbc:ID>1</cbc:ID><cbc:PaidAmount currencyID="USD">100.00</cbc:PaidAmount><cbc:PaidDate>2024-07-01</cbc:PaidDate></cac:Payment>'
        f'<cac:Payment><cbc:ID>2</cbc:ID><cbc:PaidAmount currencyID="USD">5.00</cbc:PaidAmount></cac:Payment>'
        f"{info_xml}</sac:{pre}DocumentReference>"
        for i in range(details)
    )
    body = (
        f"<cbc:UBLVersionID>2.0</cbc:UBLVersionID><cbc:ID>{id_}</cbc:ID><cbc:IssueDate>2024-07-03</cbc:IssueDate>"
        "<cac:AgentParty><cac:PartyIdentification><cbc:ID schemeID=\"6\">20000000001</cbc:ID></cac:PartyIdentification>"
        "<cac:PartyName><cbc:Name>COMERCIAL SINTETICO</cbc:Name></cac:PartyName>"
        "<cac:PostalAddress><cbc:ID>150101</cbc:ID><cbc:StreetName>AV. AGENTE 1</cbc:StreetName><cbc:CityName>LIMA</cbc:CityName>"
        "<cbc:CountrySubentity>LIMA</cbc:CountrySubentity><cbc:District>LIMA</cbc:District></cac:PostalAddress>"
        "<cac:PartyLegalEntity><cbc:RegistrationName>EMPRESA SINTETICA S.A.C.</cbc:RegistrationName></cac:PartyLegalEntity></cac:AgentParty>"
        "<cac:ReceiverParty><cac:PartyIdentification><cbc:ID schemeID=\"6\">20000000002</cbc:ID></cac:PartyIdentification>"
        "<cac:PartyLegalEntity><cbc:RegistrationName>PROVEEDOR SINTETICO</cbc:RegistrationName></cac:PartyLegalEntity></cac:ReceiverParty>"
        f"<sac:{pre}SystemCode>01</sac:{pre}SystemCode><sac:{pre}Percent>3.00</sac:{pre}Percent><cbc:Note>NOTA SINTETICA</cbc:Note>"
        f'<cbc:TotalInvoiceAmount currencyID="PEN">3.00</cbc:TotalInvoiceAmount><sac:SUNATTotal{paid} currencyID="PEN">97.00</sac:SUNATTotal{paid}>'
        + det + more
    )
    return root(kind, "ret" if kind == "Retention" else "per", body)


def case(cid: str, parser: str, xml: str | None = None, **kw) -> dict:
    c = {"id": cid, "parser": parser}
    if xml is not None:
        c["input"] = {"xml": xml}
    c.update(kw)
    return c


def file_case(cid: str, parser: str, path: str, **kw) -> dict:
    return case(cid, parser, input={"file": f"{UBL_XML}/{path}"}, **kw)


def mutations(parser: str, base: str, prefix: str) -> list[dict]:
    """Entradas inválidas comunes a todos los parsers."""
    out = [
        case(f"{prefix}-malformed", parser, base[: len(base) // 2]),
        case(f"{prefix}-empty", parser, ""),
        case(f"{prefix}-dtd-entity", parser, base.replace("<cbc:ID>", '<cbc:ID>&e;', 1).replace(DECL, DECL + '<!DOCTYPE x [<!ENTITY e "X">]>')),
        case(f"{prefix}-external-entity", parser, base.replace("<cbc:ID>", '<cbc:ID>&e;', 1).replace(DECL, DECL + '<!DOCTYPE x [<!ENTITY e SYSTEM "file:///no/existe">]>')),
        case(f"{prefix}-dom", parser, base, mode="dom"),
        case(f"{prefix}-reuse", parser, base, input2={"xml": base}),
        case(f"{prefix}-latin1", parser, input={"b64": base64.b64encode(base.replace('encoding="UTF-8"', 'encoding="ISO-8859-1"').replace("SINTETICA", "SINTÉTICA").encode("latin-1", "replace")).decode()}),
        case(f"{prefix}-no-declaration", parser, base.replace(DECL, "")),
    ]
    return out


def cases() -> list[dict]:
    out: list[dict] = []
    # ------------------------------------------------------------ factura
    inv = root("Invoice", "inv", invoice_body())
    out.append(case("invoice-full", "invoice", inv))
    out.append(case("invoice-prefixed-root", "invoice", root("Invoice", "inv", invoice_body(), prefix="inv")))
    out.append(case("invoice-minimal", "invoice", root("Invoice", "inv", "<cbc:ID>F001-1</cbc:ID>")))
    out.append(case("invoice-no-extensions", "invoice", root("Invoice", "inv", invoice_body(extensions=False))))
    out.append(case("invoice-id-without-dash", "invoice", root("Invoice", "inv", invoice_body(id_="F001123"))))
    out.append(case("invoice-id-many-dashes", "invoice", root("Invoice", "inv", invoice_body(id_="F001-1-2"))))
    out.append(case("invoice-no-issue-date", "invoice", root("Invoice", "inv", invoice_body().replace("<cbc:IssueDate>2024-01-15</cbc:IssueDate>", ""))))
    for name, d in {"offset": "2024-01-15T10:20:30-05:00", "utc": "2024-01-15T23:59:59Z", "datetime": "2024-01-15 08:00:00",
                    "invalid": "15/01/2024", "overflow": "2024-02-30", "spaces": " 2024-01-15 ", "relative": "tomorrow",
                    "date-offset": "2024-01-15-05:00", "date-z": "2024-01-15Z", "fraction": "2024-01-15T10:20:30.123456789",
                    "offset-compact": "2024-01-15T10:20:30+0530", "offset-hour": "2024-01-15T10:20:30+02", "hour-24": "2024-01-15T24:00:00",
                    "day-zero": "2024-01-00", "month-13": "2024-13-01", "day-32": "2024-01-32", "minutes-only": "2024-01-15T10:20",
                    "lower-t": "2024-01-15t10:20:30", "dst-zone": "2024-03-10T02:30:00", "year-short": "24-01-15",
                    "newline": "\n2024-01-15\n"}.items():
        out.append(case(f"invoice-date-{name}", "invoice", root("Invoice", "inv", invoice_body(date=d))))
    out.append(case("invoice-date-utc-zone", "invoice", inv, time_zone="UTC"))
    out.append(case("invoice-date-dst-new-york", "invoice", root("Invoice", "inv", invoice_body(date="2024-03-10T02:30:00")), time_zone="America/New_York"))
    for name, v in {"comma": "1,234.50", "prefix": " 12.5abc", "exp": "1e3", "empty": "", "negative-zero": "-0.00",
                    "precision": "12345678901234567.89", "plus": "+7", "hex": "0x1A", "dot": ".5",
                    "newline": "\n\t 5.5\n", "huge-exp": "1e400", "tiny-exp": "1e-40", "inf": "INF", "trailing-dot": "5.", "exp-sign": "2.5E+2x",
                    "long-fraction": "0.1234567890123456789012345678901"}.items():
        out.append(case(f"invoice-amount-{name}", "invoice", inv.replace('<cbc:PayableAmount currencyID="PEN">118.00</cbc:PayableAmount>', f'<cbc:PayableAmount currencyID="PEN">{v}</cbc:PayableAmount>')))
    out.append(case("invoice-repeated-nodes", "invoice", inv.replace("<cbc:InvoiceTypeCode>01</cbc:InvoiceTypeCode>", "<cbc:InvoiceTypeCode>03</cbc:InvoiceTypeCode><cbc:InvoiceTypeCode>01</cbc:InvoiceTypeCode>")))
    out.append(case("invoice-loose-codes", "invoice", inv.replace("<cbc:ID>1001</cbc:ID>", "<cbc:ID>1001.0</cbc:ID>").replace("<cbc:PriceTypeCode> 01 </cbc:PriceTypeCode>", "<cbc:PriceTypeCode>1</cbc:PriceTypeCode>").replace("<cbc:ChargeIndicator> false </cbc:ChargeIndicator>", "<cbc:ChargeIndicator>FALSE</cbc:ChargeIndicator>")))
    out.append(case("invoice-due-date-zero", "invoice", inv.replace("<cbc:PaymentDueDate>2024-02-15</cbc:PaymentDueDate>", "<cbc:PaymentDueDate>0</cbc:PaymentDueDate>")))
    out.append(case("invoice-whitespace-codes", "invoice", inv.replace("<cbc:ID>1001</cbc:ID>", "<cbc:ID>\n 1001 \t</cbc:ID>")))
    out.append(case("invoice-prefix-declared-deep", "invoice", root("Invoice", "inv", invoice_body(), extra_ns=COMMON.replace('xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1" ', "")).replace("<sac:AdditionalInformation>", '<sac:AdditionalInformation xmlns:sac="urn:sunat:names:specification:ubl:peru:schema:xsd:SunatAggregateComponents-1">')))
    out.append(case("invoice-other-prefixes", "invoice", root("Invoice", "inv", invoice_body().replace("cbc:", "b:").replace("cac:", "a:"), extra_ns=COMMON.replace("xmlns:cac=", "xmlns:a=").replace("xmlns:cbc=", "xmlns:b="))))
    out.append(case("invoice-unicode-cdata", "invoice", inv.replace("EMPRESA SINTETICA S.A.C.", "<![CDATA[EMPRESA «SINTÉTICA» & Ñ <S.A.C.>]]> 😀")))
    out.append(case("invoice-root-other-namespace", "invoice", inv.replace(NS["inv"], "urn:otro")))
    out.append(case("invoice-root-no-namespace", "invoice", inv.replace(f'xmlns="{NS["inv"]}" ', "")))
    out.append(case("invoice-zero-lines", "invoice", root("Invoice", "inv", invoice_body(lines=0))))
    out.append(case("invoice-many-lines", "invoice", root("Invoice", "inv", invoice_body(lines=40))))
    out += mutations("invoice", inv, "invoice")
    for path in ("invoice/inv21-full-default.xml", "invoice/inv20-full-default.xml", "invoice/boleta21-default.xml"):
        out.append(file_case("invoice-ubl-" + path.split("/")[1][:-4], "invoice", path))
    # ------------------------------------------------------------- notas
    ncr = root("CreditNote", "cn", note_body(True))
    ndb = root("DebitNote", "dn", note_body(False, id_="FD01-6"))
    out.append(case("note-credit-full", "note", ncr))
    out.append(case("note-debit-full", "note", ndb))
    out.append(case("note-credit-prefixed-root", "note", root("CreditNote", "cn", note_body(True), prefix="cn")))
    out.append(case("note-credit-no-extensions", "note", root("CreditNote", "cn", note_body(True, extensions=False))))
    out.append(case("note-credit-many-lines", "note", root("CreditNote", "cn", note_body(True, lines=3))))
    out.append(case("note-missing-quantity", "note", ncr.replace('<cbc:CreditedQuantity unitCode="NIU">2</cbc:CreditedQuantity>', "")))
    out.append(case("note-total-without-id", "note", ncr.replace("<cbc:ID>1001</cbc:ID>", "")))
    out.append(case("note-id-without-dash", "note", root("CreditNote", "cn", note_body(True, id_="FC015"))))
    out.append(case("note-billing-reference-nested", "note", ncr.replace("<cac:BillingReference>", "<cac:Signature><cac:BillingReference><cac:InvoiceDocumentReference><cbc:DocumentTypeCode>99</cbc:DocumentTypeCode></cac:InvoiceDocumentReference></cac:BillingReference></cac:Signature><cac:BillingReference>")))
    out += mutations("note", ncr, "note")
    for path in ("note/cr21-full-default.xml", "note/db20-full-default.xml"):
        out.append(file_case("note-ubl-" + path.split("/")[1][:-4], "note", path))
    # ------------------------------------------------------------ guías
    des = root("DespatchAdvice", "da", despatch_body())
    out.append(case("despatch-full", "despatch", des))
    out.append(case("despatch-no-third-no-shipment", "despatch", root("DespatchAdvice", "da", despatch_body(third=False, shipment=False))))
    out.append(case("despatch-no-transfer-indicator", "despatch", des.replace("<cbc:SplitConsignmentIndicator>true</cbc:SplitConsignmentIndicator>", "")))
    out.append(case("despatch-indicator-upper", "despatch", des.replace(">true</cbc:SplitConsignmentIndicator>", ">TRUE</cbc:SplitConsignmentIndicator>")))
    out.append(case("despatch-bultos-text", "despatch", des.replace(">3</cbc:TotalTransportHandlingUnitQuantity>", ">3.9 bultos</cbc:TotalTransportHandlingUnitQuantity>")))
    out.append(case("despatch-bultos-exp", "despatch", des.replace(">3</cbc:TotalTransportHandlingUnitQuantity>", "> 1e3</cbc:TotalTransportHandlingUnitQuantity>")))
    out.append(case("despatch-bultos-negative", "despatch", des.replace(">3</cbc:TotalTransportHandlingUnitQuantity>", ">-4</cbc:TotalTransportHandlingUnitQuantity>")))
    out.append(case("despatch-missing-quantity", "despatch", des.replace('<cbc:DeliveredQuantity unitCode="NIU">1.5</cbc:DeliveredQuantity>', "")))
    out.append(case("despatch-carrier-without-id", "despatch", des.replace('<cac:PartyIdentification><cbc:ID schemeID="6">20000000004</cbc:ID></cac:PartyIdentification>', "")))
    out.append(case("despatch-no-lines", "despatch", root("DespatchAdvice", "da", despatch_body(lines=0))))
    out += mutations("despatch", des, "despatch")
    for path in ("despatch/desp-full-default.xml", "despatch/desp2022-full-default.xml"):
        out.append(file_case("despatch-ubl-" + path.split("/")[1][:-4], "despatch", path))
    # --------------------------------------------------- recibo honorarios
    rec = receipt_xml()
    out.append(case("receipt-full", "receipt", rec))
    out.append(case("receipt-minimal", "receipt", root("Invoice", "inv", "<cbc:ID>E001-1</cbc:ID>")))
    out += mutations("receipt", rec, "receipt")
    # ----------------------------------------------------------- resumen
    sm = summary_xml()
    out.append(case("summary-full", "summary", sm))
    out.append(case("summary-loose-codes", "summary", sm.replace("<cbc:InstructionID>01</cbc:InstructionID>", "<cbc:InstructionID>1</cbc:InstructionID>").replace("<cbc:InstructionID> 05 </cbc:InstructionID>", "<cbc:InstructionID>5.0</cbc:InstructionID>")))
    out.append(case("summary-id-two-parts", "summary", summary_xml(id_="RC-20240601")))
    out.append(case("summary-no-lines", "summary", root("SummaryDocuments", "sd", "<cbc:ID>RC-20240601-2</cbc:ID>")))
    out += mutations("summary", sm, "summary")
    out.append(file_case("summary-ubl-summary-full-default", "summary", "summary/summary-full-default.xml"))
    # ----------------------------------------------------------- bajas
    vo = voided_xml()
    out.append(case("voided-ra", "voided", vo))
    out.append(case("voided-rr", "voided", voided_xml(id_="RR-20240601-8")))
    out.append(case("voided-unknown-prefix", "voided", voided_xml(id_="XX-20240601-9")))
    out.append(case("voided-lowercase-ra", "voided", voided_xml(id_="ra-20240601-9")))
    out.append(case("voided-id-without-dash", "voided", voided_xml(id_="RA20240601")))
    out += mutations("voided", vo, "voided")
    for path in ("voided/voided-full-default.xml", "voided/reversion-full-default.xml"):
        out.append(file_case("voided-ubl-" + path.split("/")[1][:-4], "voided", path))
    # ----------------------------------------------- retención/percepción
    for kind, parser, serie in (("Retention", "retention", "R001-1"), ("Perception", "perception", "P001-1")):
        x = cp_xml(kind, serie)
        out.append(case(f"{parser}-full", parser, x))
        out.append(case(f"{parser}-without-information", parser, cp_xml(kind, serie, info=False)))
        out.append(case(f"{parser}-without-exchange", parser, cp_xml(kind, serie, exchange=False, details=2)))
        out.append(case(f"{parser}-paid-date-at-root", parser, x.replace("<cbc:IssueDate>2024-07-03</cbc:IssueDate>", "<cbc:IssueDate>2024-07-03</cbc:IssueDate><cbc:PaidDate>2024-07-09</cbc:PaidDate>")))
        out.append(case(f"{parser}-missing-receiver-id", parser, x.replace('<cac:PartyIdentification><cbc:ID schemeID="6">20000000002</cbc:ID></cac:PartyIdentification>', "")))
        out += mutations(parser, x, parser)
        name = "retention" if kind == "Retention" else "perception"
        out.append(file_case(f"{parser}-ubl-{name}-full-default", parser, f"retention-perception/{name}-full-default.xml"))
    # -------------------------------------------------- límites y hostiles
    deep = DECL + "<Invoice xmlns=\"urn:oasis:names:specification:ubl:schema:xsd:Invoice-2\">" + "<a>" * 300 + "x" + "</a>" * 300 + "</Invoice>"
    out.append(case("hostile-deep-300", "invoice", deep))
    deep100 = inv.replace("</Invoice>", "<a>" * 100 + "x" + "</a>" * 100 + "</Invoice>")
    out.append(case("hostile-deep-100", "invoice", deep100))
    out.append(case("hostile-billion-laughs", "voided", DECL + '<!DOCTYPE r [<!ENTITY a "lol">' + "".join(f'<!ENTITY {chr(98+i)} "{("&" + chr(97+i) + ";") * 10}">' for i in range(9)) + "]><VoidedDocuments><cbc:ID>&j;</cbc:ID></VoidedDocuments>"))
    out.append(case("hostile-huge-11mib", "voided", input={"repeat": {"prefix": vo[: vo.index("</VoidedDocuments>")], "unit": "<x>" + "a" * 1000 + "</x>", "count": 11 * 1024, "suffix": "</VoidedDocuments>"}}))
    out.append(case("hostile-utf16", "voided", input={"b64": base64.b64encode(vo.replace('encoding="UTF-8"', 'encoding="UTF-16"').encode("utf-16")).decode()}))
    out.append(case("hostile-koi8r", "voided", vo.replace('encoding="UTF-8"', 'encoding="KOI8-R"')))
    out.append(case("hostile-bad-utf8", "voided", input={"b64": base64.b64encode(vo.replace("ERROR SINTETICO 1", "ERROR \x00").encode().replace(b"\x00", b"\xff")).decode()}))
    out.append(case("hostile-cp1252", "voided", input={"b64": base64.b64encode(vo.replace('encoding="UTF-8"', 'encoding="windows-1252"').replace("ERROR SINTETICO 1", "ERROR “€”").encode("cp1252")).decode()}))
    # ------------------------------------------------------------- lector
    reader_ops = [
        {"op": "value", "query": "cbc:ID"},
        {"op": "value", "query": "cbc:NoExiste"},
        {"op": "value", "query": "cbc:NoExiste", "default": None},
        {"op": "value", "query": "cbc:NoExiste", "default": "def"},
        {"op": "value", "query": "/inv:Invoice/cbc:ID"},
        {"op": "value", "query": "//cbc:CityName"},
        {"op": "value", "query": "cac:Party/cac:PartyName/cbc:Name", "context": "cac:AccountingSupplierParty"},
        {"op": "value", "query": "cac:AccountingSupplierParty"},
        {"op": "value", "query": "cbc:PayableAmount/@currencyID", "context": "cac:LegalMonetaryTotal"},
        {"op": "value", "query": "zz:ID"},
        {"op": "value", "query": "count(cac:InvoiceLine)"},
        {"op": "node", "query": "cac:LegalMonetaryTotal"},
        {"op": "node", "query": "cbc:PayableAmount/@currencyID", "context": "cac:LegalMonetaryTotal"},
        {"op": "node", "query": "cbc:Nada"},
        {"op": "nodes", "query": "cac:InvoiceLine"},
        {"op": "nodes", "query": "cac:Nada"},
        {"op": "nodes", "query": "zz:x"},
        {"op": "nodes", "query": "//cbc:ID"},
        {"op": "nodes", "query": "cac:Nada/zz:x"},
        {"op": "value", "query": "cac:LegalMonetaryTotal/cbc:PayableAmount"},
        {"op": "value", "query": "cbc:ID", "context": "cac:InvoiceLine"},
        {"op": "nodes", "query": "cac:InvoiceLine//cbc:ID"},
        {"op": "value", "query": "Invoice"},
    ]
    reader_doc = inv.replace(f'xmlns="{NS["inv"]}"', f'xmlns="{NS["inv"]}" xmlns:inv="{NS["inv"]}"')
    out.append(case("reader-api", "reader", reader_doc, reader=reader_ops))
    return out


def main() -> None:
    target = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "tests/reference/xml-parser/cases.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    data = cases()
    ids = [c["id"] for c in data]
    if len(ids) != len(set(ids)):
        raise SystemExit("ids duplicados: " + str([i for i in ids if ids.count(i) > 1]))
    target.write_text(json.dumps({"cases": data}, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
