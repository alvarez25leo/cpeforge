# Matriz de campos — cpeforge-xml-parser 0.1.0

Generada con `python3 -I oracle/cases/xml_parser/matrix.py` desde las fuentes PHP fijadas
(`f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f`) y `tests/reference/xml-parser/expected.json`.
Cada fila es una llamada a un setter en el orden del código: la expresión PHP literal
(ruta XPath, nodo de contexto, valor por defecto y conversión) y el número de
objetos del oráculo en que una propiedad con ese nombre tiene valor PHP no nulo (todas las
clases y casos, por eso `tipoDoc` o `ruc` suman varias familias). La prueba
`crates/xml-parser/tests/parity.rs` compara el modelo completo campo a campo y falla si una
propiedad asignada por un setter no se compara nunca con un valor no nulo.

## DespatchParser

| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |
| --- | --- | --- | --- | --- |
| 57 | `serie` | `$docGuia[0]` | texto | 164 |
| 58 | `correlativo` | `$docGuia[1]` | texto | 188 |
| 59 | `tipoDoc` | `$xml->getValue('cbc:DespatchAdviceTypeCode', $root)` | texto | 513 |
| 60 | `observacion` | `$xml->getValue('cbc:Note', $root)` | texto | 42 |
| 61 | `fechaEmision` | `new DateTime($xml->getValue('cbc:IssueDate', $root))` | `new DateTime` | 154 |
| 62 | `company` | `$this->getCompany()` | texto | 152 |
| 63 | `destinatario` | `$this->getClient('cac:DeliveryCustomerParty')` | texto | 16 |
| 64 | `tercero` | `$this->getClient('cac:SellerSupplierParty')` | texto | 15 |
| 65 | `envio` | `$this->getShipment()` | texto | 18 |
| 66 | `details` | `iterator_to_array($this->getDetails())` | texto | 152 |
| 84 | `numDoc` | `$docNode->nodeValue` | texto | 183 |
| 85 | `tipoDoc` | `$docNode->getAttribute('schemeID')` | texto | 513 |
| 86 | `rznSocial` | `$xml->getValue('cac:Party/cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 157 |
| 97 | `ruc` | `$xml->getValue('cbc:CustomerAssignedAccountID', $node)` | texto | 162 |
| 98 | `razonSocial` | `$xml->getValue('cac:Party/cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 162 |
| 109 | `tipoDoc` | `$xml->getValue('cbc:OrderTypeCode', $bajaNode)` | texto | 513 |
| 110 | `nroDoc` | `$xml->getValue('cbc:ID', $bajaNode)` | texto | 177 |
| 111 | `docBaja` | `$baja` | texto | 16 |
| 116 | `tipoDoc` | `$xml->getValue('cbc:OrderTypeCode', $relDoc)` | texto | 513 |
| 117 | `nroDoc` | `$xml->getValue('cbc:ID', $relDoc)` | texto | 177 |
| 118 | `relDoc` | `$rel` | texto | 17 |
| 128 | `codTraslado` | `$xml->getValue('cbc:HandlingCode', $node)` | texto | 18 |
| 129 | `desTraslado` | `$xml->getValue('cbc:Information', $node)` | texto | 18 |
| 130 | `numBultos` | `(int)$xml->getValue('cbc:TotalTransportHandlingUnitQuantity', $node, '0')` | `(int)` | 18 |
| 131 | `indTransbordo` | `$xml->getValue('cbc:SplitConsignmentIndicator', $node, 'false') == 'true'` | `== 'true'` | 18 |
| 132 | `numContenedor` | `$xml->getValue('cac:TransportHandlingUnit/cbc:ID', $node)` | texto | 18 |
| 133 | `codPuerto` | `$xml->getValue('cac:FirstArrivalPortLocation/cbc:ID', $node)` | texto | 18 |
| 137 | `undPesoTotal` | `$otNode->getAttribute('unitCode')` | texto | 16 |
| 138 | `pesoTotal` | `(float)$otNode->nodeValue` | `(float)` → Decimal | 16 |
| 142 | `llegada` | `new Direction($xml->getValue('cbc:ID', $otNode), $xml->getValue('cbc:StreetName', $otNode))` | texto | 16 |
| 146 | `partida` | `new Direction($xml->getValue('cbc:ID', $otNode), $xml->getValue('cbc:StreetName', $otNode))` | texto | 15 |
| 150 | `modTraslado` | `$xml->getValue('cbc:TransportModeCode', $otNode)` | texto | 18 |
| 151 | `fecTraslado` | `new DateTime($xml->getValue('cac:TransitPeriod/cbc:StartDate', $otNode))` | `new DateTime` | 18 |
| 152 | `transportista` | `$this->getTransportista($otNode)` | texto | 18 |
| 168 | `tipoDoc` | `$idNode->getAttribute('schemeID')` | texto | 513 |
| 169 | `numDoc` | `$idNode->nodeValue` | texto | 183 |
| 170 | `rznSocial` | `$xml->getValue('cac:PartyName/cbc:Name', $othNode)` | texto | 157 |
| 173 | `placa` | `$xml->getValue('cac:TransportMeans/cac:RoadTransport/cbc:LicensePlateID', $node)` | texto | 18 |
| 176 | `choferTipoDoc` | `$othNode->getAttribute('schemeID')` | texto | 16 |
| 177 | `choferDoc` | `$othNode->nodeValue` | texto | 16 |
| 191 | `cantidad` | `(float)$quantity->nodeValue` | `(float)` → Decimal | 205 |
| 192 | `unidad` | `$quantity->getAttribute('unitCode')` | texto | 205 |
| 193 | `descripcion` | `$xml->getValue('cac:Item/cbc:Name', $node)` | texto | 205 |
| 194 | `codigo` | `$xml->getValue('cac:Item/cac:SellersItemIdentification/cbc:ID', $node)` | texto | 32 |
| 195 | `codProdSunat` | `$xml->getValue('cac:Item/cac:CommodityClassification/cbc:ItemClassificationCode', $node)` | texto | 205 |

## InvoiceParser

| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |
| --- | --- | --- | --- | --- |
| 47 | `serie` | `$docFac[0]` | texto | 164 |
| 48 | `correlativo` | `$docFac[1]` | texto | 188 |
| 49 | `tipoDoc` | `$this->defValue($xpt->query('/xt:Invoice/cbc:InvoiceTypeCode'))` | texto | 513 |
| 50 | `tipoMoneda` | `$this->defValue($xpt->query('/xt:Invoice/cbc:DocumentCurrencyCode'))` | texto | 76 |
| 51 | `fechaEmision` | `new DateTime($this->defValue($xpt->query('/xt:Invoice/cbc:IssueDate')))` | `new DateTime` | 154 |
| 52 | `company` | `$this->getCompany($xpt)` | texto | 152 |
| 53 | `client` | `$this->getClient($xpt)` | texto | 76 |
| 60 | `tipoOperacion` | `$this->defValue($xpt->query('sac:SUNATTransaction/cbc:ID', $additional))` | texto | 60 |
| 61 | `sumDsctoGlobal` | `(float)$this->defValue($xpt->query('cbc:AllowanceTotalAmount', $monetaryTotal), '0')` | `(float)` → Decimal | 60 |
| 62 | `totalAnticipos` | `(float)$this->defValue($xpt->query('cbc:PrepaidAmount', $monetaryTotal), '0')` | `(float)` → Decimal | 60 |
| 63 | `anticipos` | `iterator_to_array($this->getPrepayments($xpt))` | texto | 60 |
| 64 | `mtoOtrosTributos` | `(float)$this->defValue($xpt->query('cbc:ChargeTotalAmount', $monetaryTotal), '0')` | `(float)` → Decimal | 87 |
| 65 | `mtoImpVenta` | `(float)$this->defValue($xpt->query('cbc:PayableAmount', $monetaryTotal), '0')` | `(float)` → Decimal | 76 |
| 66 | `details` | `iterator_to_array($this->getDetails($xpt))` | texto | 152 |
| 67 | `legends` | `iterator_to_array($this->getLegends($xpt, $additional))` | texto | 76 |
| 124 | `mtoOperGravadas` | `$val` | texto | 82 |
| 127 | `mtoOperInafectas` | `$val` | texto | 68 |
| 130 | `mtoOperExoneradas` | `$val` | texto | 68 |
| 133 | `mtoOperGratuitas` | `$val` | texto | 67 |
| 136 | `perception` | `(new SalePerception())` | texto | 72 |
| 137 | `codReg` | `$this->defNodeAttribute($xpt->query('cbc:ID', $total), 'schemeID')` | texto | 83 |
| 138 | `mto` | `$val` | texto | 83 |
| 139 | `mtoBase` | `(float)$this->defValue($xpt->query('sac:ReferenceAmount', $total), '0')` | `(float)` → Decimal | 83 |
| 140 | `mtoTotal` | `(float)$this->defValue($xpt->query('sac:TotalAmount', $total), '0')` | `(float)` → Decimal | 83 |
| 143 | `detraccion` | `(new Detraction())` | texto | 58 |
| 144 | `mount` | `$val` | texto | 58 |
| 145 | `percent` | `(float)$this->defValue($xpt->query('cbc:Percent', $total), '0')` | `(float)` → Decimal | 58 |
| 146 | `valueRef` | `(float)$this->defValue($xpt->query('sac:ReferenceAmount', $total), '0')` | `(float)` → Decimal | 58 |
| 149 | `mtoDescuentos` | `$val` | texto | 58 |
| 163 | `mtoIGV` | `$val` | texto | 87 |
| 166 | `mtoISC` | `$val` | texto | 69 |
| 169 | `sumOtrosCargos` | `$val` | texto | 74 |
| 183 | `total` | `(float)$this->defValue($xpt->query('cbc:PaidAmount', $node), '0')` | `(float)` → Decimal | 150 |
| 184 | `tipoDocRel` | `$this->defNodeAttribute($xpt->query('cbc:ID', $node), 'schemeID')` | texto | 117 |
| 185 | `nroDocRel` | `$this->defValue($xpt->query('cbc:ID', $node))` | texto | 117 |
| 199 | `code` | `$this->defValue($xpt->query('cbc:ID', $legend))` | texto | 144 |
| 200 | `value` | `$this->defValue($xpt->query('cbc:Value', $legend))` | texto | 144 |
| 209 | `numDoc` | `$this->defValue($xp->query('cbc:CustomerAssignedAccountID', $node))` | texto | 183 |
| 210 | `tipoDoc` | `$this->defValue($xp->query('cbc:AdditionalAccountID', $node))` | texto | 513 |
| 211 | `rznSocial` | `$this->defValue($xp->query('cac:Party/cac:PartyLegalEntity/cbc:RegistrationName', $node))` | texto | 157 |
| 212 | `address` | `$this->getAddress($xp, $node)` | texto | 178 |
| 222 | `ruc` | `$this->defValue($xp->query('cbc:CustomerAssignedAccountID', $node))` | texto | 162 |
| 223 | `nombreComercial` | `$this->defValue($xp->query('cac:Party/cac:PartyName/cbc:Name', $node))` | texto | 134 |
| 224 | `razonSocial` | `$this->defValue($xp->query('cac:Party/cac:PartyLegalEntity/cbc:RegistrationName', $node))` | texto | 162 |
| 225 | `address` | `$this->getAddress($xp, $node)` | texto | 178 |
| 232 | `compra` | `$this->defValue($xpt->query('/xt:Invoice/cac:OrderReference/cbc:ID'))` | texto | 60 |
| 235 | `fecVencimiento` | `new DateTime($fecVen)` | `new DateTime` | 58 |
| 238 | `guias` | `iterator_to_array($this->getGuias($xpt))` | texto | 76 |
| 250 | `tipoDoc` | `$this->defValue($xpt->query('cbc:DocumentTypeCode', $guia))` | texto | 513 |
| 251 | `nroDoc` | `$this->defValue($xpt->query('cbc:ID', $guia))` | texto | 177 |
| 269 | `direccion` | `$this->defValue($xp->query('cbc:StreetName', $address))` | texto | 209 |
| 270 | `departamento` | `$this->defValue($xp->query('cbc:CityName', $address))` | texto | 158 |
| 271 | `provincia` | `$this->defValue($xp->query('cbc:CountrySubentity', $address))` | texto | 158 |
| 272 | `distrito` | `$this->defValue($xp->query('cbc:District', $address))` | texto | 158 |
| 273 | `ubigueo` | `$this->defValue($xp->query('cbc:ID', $address))` | texto | 189 |
| 285 | `cantidad` | `(float)$this->defValue($xpt->query('cbc:InvoicedQuantity', $node), '0')` | `(float)` → Decimal | 205 |
| 286 | `unidad` | `$this->defNodeAttribute($xpt->query('cbc:InvoicedQuantity', $node), 'unitCode')` | texto | 205 |
| 287 | `mtoValorVenta` | `(float)$this->defValue($xpt->query('cbc:LineExtensionAmount', $node))` | `(float)` → Decimal | 173 |
| 288 | `mtoValorUnitario` | `(float)$this->defValue($xpt->query('cac:Price/cbc:PriceAmount', $node))` | `(float)` → Decimal | 173 |
| 289 | `descripcion` | `$this->defValue($xpt->query('cac:Item/cbc:Description', $node))` | texto | 205 |
| 290 | `codProducto` | `$this->defValue($xpt->query('cac:Item/cac:SellersItemIdentification/cbc:ID', $node))` | texto | 173 |
| 291 | `codProdSunat` | `$this->defValue($xpt->query('cac:Item/cac:CommodityClassification/cbc:ItemClassificationCode', $node))` | texto | 205 |
| 309 | `igv` | `$val` | texto | 173 |
| 310 | `tipAfeIgv` | `$this->defValue($xpt->query('cac:TaxSubtotal/cac:TaxCategory/cbc:TaxExemptionReasonCode', $tax))` | texto | 173 |
| 313 | `isc` | `$val` | texto | 167 |
| 314 | `tipSisIsc` | `$this->defValue($xpt->query('cac:TaxSubtotal/cac:TaxCategory/cbc:TierRange', $tax))` | texto | 167 |
| 328 | `descuento` | `$val` | texto | 167 |
| 343 | `mtoPrecioUnitario` | `$value` | texto | 173 |
| 346 | `mtoValorGratuito` | `$value` | texto | 167 |

## NoteParser

| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |
| --- | --- | --- | --- | --- |
| 61 | `serie` | `$idNum[0]` | texto | 164 |
| 62 | `correlativo` | `$idNum[1]` | texto | 188 |
| 63 | `tipoDoc` | `$isNcr ? '07' : '08'` | texto | 513 |
| 64 | `tipoMoneda` | `$xml->getValue('cbc:DocumentCurrencyCode', $root)` | texto | 76 |
| 65 | `fechaEmision` | `new DateTime($xml->getValue('cbc:IssueDate', $root))` | `new DateTime` | 154 |
| 66 | `company` | `$this->getCompany()` | texto | 152 |
| 67 | `client` | `$this->getClient()` | texto | 76 |
| 74 | `mtoOtrosTributos` | `(float)$xml->getValue('cbc:ChargeTotalAmount', $monetaryTotal, '0')` | `(float)` → Decimal | 87 |
| 75 | `mtoImpVenta` | `(float)$xml->getValue('cbc:PayableAmount', $monetaryTotal, '0')` | `(float)` → Decimal | 76 |
| 76 | `details` | `iterator_to_array($this->getDetails($isNcr))` | texto | 152 |
| 77 | `legends` | `iterator_to_array($this->getLegends($additional))` | texto | 76 |
| 78 | `guias` | `iterator_to_array($this->getGuias($root))` | texto | 76 |
| 98 | `mtoOperGravadas` | `$val` | texto | 82 |
| 101 | `mtoOperInafectas` | `$val` | texto | 68 |
| 104 | `mtoOperExoneradas` | `$val` | texto | 68 |
| 107 | `mtoOperGratuitas` | `$val` | texto | 67 |
| 110 | `perception` | `(new SalePerception())` | texto | 72 |
| 111 | `codReg` | `$nodeId->getAttribute('schemeID')` | texto | 83 |
| 112 | `mto` | `$val` | texto | 83 |
| 113 | `mtoBase` | `(float)$xml->getValue('sac:ReferenceAmount', $total, '0')` | `(float)` → Decimal | 83 |
| 114 | `mtoTotal` | `(float)$xml->getValue('sac:TotalAmount', $total, '0')` | `(float)` → Decimal | 83 |
| 130 | `mtoIGV` | `$val` | texto | 87 |
| 133 | `mtoISC` | `$val` | texto | 69 |
| 136 | `sumOtrosCargos` | `$val` | texto | 74 |
| 153 | `code` | `$xml->getValue('cbc:ID', $legend)` | texto | 144 |
| 154 | `value` | `$xml->getValue('cbc:Value', $legend)` | texto | 144 |
| 164 | `codMotivo` | `$xml->getValue('cbc:ResponseCode', $node)` | texto | 16 |
| 165 | `desMotivo` | `$xml->getValue('cbc:Description', $node)` | texto | 16 |
| 166 | `numDocfectado` | `$xml->getValue('cbc:ReferenceID', $node)` | texto | 16 |
| 167 | `tipDocAfectado` | `$xml->getValue('cac:BillingReference/cac:InvoiceDocumentReference/cbc:DocumentTypeCode')` | texto | 16 |
| 176 | `numDoc` | `$xml->getValue('cbc:CustomerAssignedAccountID', $node)` | texto | 183 |
| 177 | `tipoDoc` | `$xml->getValue('cbc:AdditionalAccountID', $node)` | texto | 513 |
| 178 | `rznSocial` | `$xml->getValue('cac:Party/cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 157 |
| 179 | `address` | `$this->getAddress($node)` | texto | 178 |
| 190 | `ruc` | `$xml->getValue('cbc:CustomerAssignedAccountID', $node)` | texto | 162 |
| 191 | `nombreComercial` | `$xml->getValue('cac:Party/cac:PartyName/cbc:Name', $node)` | texto | 134 |
| 192 | `razonSocial` | `$xml->getValue('cac:Party/cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 162 |
| 193 | `address` | `$this->getAddress($node)` | texto | 178 |
| 208 | `tipoDoc` | `$xml->getValue('cbc:DocumentTypeCode', $guia)` | texto | 513 |
| 209 | `nroDoc` | `$xml->getValue('cbc:ID', $guia)` | texto | 177 |
| 225 | `direccion` | `$xml->getValue('cbc:StreetName', $address)` | texto | 209 |
| 226 | `departamento` | `$xml->getValue('cbc:CityName', $address)` | texto | 158 |
| 227 | `provincia` | `$xml->getValue('cbc:CountrySubentity', $address)` | texto | 158 |
| 228 | `distrito` | `$xml->getValue('cbc:District', $address)` | texto | 158 |
| 229 | `ubigueo` | `$xml->getValue('cbc:ID', $address)` | texto | 189 |
| 243 | `cantidad` | `(float)$quant->nodeValue` | `(float)` → Decimal | 205 |
| 244 | `unidad` | `$quant->getAttribute('unitCode')` | texto | 205 |
| 245 | `mtoValorVenta` | `(float)$xml->getValue('cbc:LineExtensionAmount', $node)` | `(float)` → Decimal | 173 |
| 246 | `mtoValorUnitario` | `(float)$xml->getValue('cac:Price/cbc:PriceAmount', $node)` | `(float)` → Decimal | 173 |
| 247 | `descripcion` | `$xml->getValue('cac:Item/cbc:Description', $node)` | texto | 205 |
| 248 | `codProducto` | `$xml->getValue('cac:Item/cac:SellersItemIdentification/cbc:ID', $node)` | texto | 173 |
| 249 | `codProdSunat` | `$xml->getValue('cac:Item/cac:CommodityClassification/cbc:ItemClassificationCode', $node)` | texto | 205 |
| 269 | `igv` | `$val` | texto | 173 |
| 270 | `tipAfeIgv` | `$xml->getValue('cac:TaxSubtotal/cac:TaxCategory/cbc:TaxExemptionReasonCode', $tax)` | texto | 173 |
| 273 | `isc` | `$val` | texto | 167 |
| 274 | `tipSisIsc` | `$xml->getValue('cac:TaxSubtotal/cac:TaxCategory/cbc:TierRange', $tax)` | texto | 167 |
| 288 | `descuento` | `$val` | texto | 167 |
| 303 | `mtoPrecioUnitario` | `$value` | texto | 173 |
| 306 | `mtoValorGratuito` | `$value` | texto | 167 |

## PerceptionParser

| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |
| --- | --- | --- | --- | --- |
| 57 | `serie` | `$idNum[0]` | texto | 164 |
| 58 | `correlativo` | `$idNum[1]` | texto | 188 |
| 59 | `fechaEmision` | `new DateTime($xml->getValue('cbc:IssueDate'))` | `new DateTime` | 154 |
| 60 | `company` | `$this->getCompany()` | texto | 152 |
| 61 | `proveedor` | `$this->getClient()` | texto | 24 |
| 62 | `regimen` | `$xml->getValue('sac:SUNATPerceptionSystemCode')` | texto | 24 |
| 63 | `tasa` | `(float)$xml->getValue('sac:SUNATPerceptionPercent', $root, '0')` | `(float)` → Decimal | 35 |
| 64 | `observacion` | `$xml->getValue('cbc:Note')` | texto | 42 |
| 65 | `impPercibido` | `(float)$xml->getValue('cbc:TotalInvoiceAmount', $root, '0')` | `(float)` → Decimal | 25 |
| 66 | `impCobrado` | `(float)$xml->getValue('sac:SUNATTotalCashed', $root, '0')` | `(float)` → Decimal | 12 |
| 67 | `details` | `iterator_to_array($this->getDetails())` | texto | 152 |
| 78 | `ruc` | `$xml->getValue('cac:PartyIdentification/cbc:ID', $node)` | texto | 162 |
| 79 | `nombreComercial` | `$xml->getValue('cac:PartyName/cbc:Name', $node)` | texto | 134 |
| 80 | `razonSocial` | `$xml->getValue('cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 162 |
| 81 | `address` | `$this->getAddress($node)` | texto | 178 |
| 93 | `numDoc` | `$ident->nodeValue` | texto | 183 |
| 94 | `tipoDoc` | `$ident->getAttribute('schemeID')` | texto | 513 |
| 95 | `rznSocial` | `$xml->getValue('cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 157 |
| 96 | `address` | `$this->getAddress($node)` | texto | 178 |
| 111 | `direccion` | `$xml->getValue('cbc:StreetName', $address)` | texto | 209 |
| 112 | `departamento` | `$xml->getValue('cbc:CityName', $address)` | texto | 158 |
| 113 | `provincia` | `$xml->getValue('cbc:CountrySubentity', $address)` | texto | 158 |
| 114 | `distrito` | `$xml->getValue('cbc:District', $address)` | texto | 158 |
| 115 | `ubigueo` | `$xml->getValue('cbc:ID', $address)` | texto | 189 |
| 131 | `tipoDoc` | `$temp->getAttribute('schemeID')` | texto | 513 |
| 132 | `numDoc` | `$temp->nodeValue` | texto | 183 |
| 133 | `fechaEmision` | `new DateTime($xml->getValue('cbc:IssueDate', $node))` | `new DateTime` | 154 |
| 134 | `impTotal` | `(float)$mount->nodeValue` | `(float)` → Decimal | 26 |
| 135 | `moneda` | `$mount->getAttribute('currencyID')` | texto | 90 |
| 136 | `cobros` | `iterator_to_array($this->getPayments($node))` | texto | 13 |
| 140 | `impPercibido` | `0.00` | texto | 25 |
| 141 | `impCobrar` | `0.00` | texto | 13 |
| 142 | `fechaPercepcion` | `new DateTime()` | `new DateTime` | 13 |
| 149 | `impPercibido` | `(float)$xml->getValue('sac:SUNATPerceptionAmount', $temp)` | `(float)` → Decimal | 25 |
| 150 | `fechaPercepcion` | `new DateTime($xml->getValue('sac:SUNATPerceptionDate', $temp))` | `new DateTime` | 13 |
| 151 | `impCobrar` | `(float)$xml->getValue('sac:SUNATNetTotalCashed', $temp)` | `(float)` → Decimal | 13 |
| 156 | `monedaRef` | `$xml->getValue('cbc:SourceCurrencyCode', $cambio)` | texto | 20 |
| 157 | `monedaObj` | `$xml->getValue('cbc:TargetCurrencyCode', $cambio)` | texto | 20 |
| 158 | `factor` | `(float)$xml->getValue('cbc:CalculationRate', $cambio, '0')` | `(float)` → Decimal | 20 |
| 159 | `fecha` | `new DateTime($xml->getValue('cbc:Date', $cambio))` | `new DateTime` | 71 |
| 160 | `tipoCambio` | `$exc` | texto | 20 |
| 175 | `moneda` | `$temp->getAttribute('currencyID')` | texto | 90 |
| 176 | `importe` | `(float)$temp->nodeValue` | `(float)` → Decimal | 51 |
| 177 | `fecha` | `new DateTime($xml->getValue('cbc:PaidDate'))` | `new DateTime` | 71 |

## ReceiptParser

| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |
| --- | --- | --- | --- | --- |
| 55 | `serie` | `$docFac[0]` | texto | 164 |
| 56 | `correlativo` | `$docFac[1]` | texto | 188 |
| 57 | `fechaEmision` | `new DateTime($xml->getValue('cbc:IssueDate', $root))` | `new DateTime` | 154 |
| 58 | `montoLetras` | `$xml->getValue('cbc:Note', $root)` | texto | 10 |
| 59 | `person` | `$this->getPerson()` | texto | 10 |
| 60 | `receptor` | `$this->getClient()` | texto | 10 |
| 63 | `subTotal` | `(float)$xml->getValue('cac:TaxTotal/cac:TaxSubtotal/cbc:TaxableAmount', $root, '0')` | `(float)` → Decimal | 10 |
| 64 | `retencion` | `(float)$xml->getValue('cbc:TaxExclusiveAmount', $monetaryTotal, '0')` | `(float)` → Decimal | 10 |
| 65 | `total` | `(float)$xml->getValue('cbc:PayableAmount', $monetaryTotal, '0')` | `(float)` → Decimal | 150 |
| 78 | `numDoc` | `$xml->getValue('cbc:CustomerAssignedAccountID', $node)` | texto | 183 |
| 79 | `tipoDoc` | `$xml->getValue('cbc:AdditionalAccountID', $node)` | texto | 513 |
| 80 | `rznSocial` | `$xml->getValue('cac:Party/cac:PartyName/cbc:Name', $node)` | texto | 157 |
| 81 | `address` | `$this->getAddress($node)` | texto | 178 |
| 92 | `ruc` | `$xml->getValue('cbc:CustomerAssignedAccountID', $node)` | texto | 162 |
| 93 | `razonSocial` | `$xml->getValue('cac:Party/cac:PartyName/cbc:Name', $node)` | texto | 162 |
| 94 | `address` | `$this->getAddress($node)` | texto | 178 |
| 106 | `direccion` | `$xml->getValue('cac:Party/cac:PostalAddress/cbc:StreetName', $node)` | texto | 209 |
| 117 | `porcentaje` | `(float)$xml->getValue('cac:TaxTotal/cac:TaxSubtotal/cbc:Percent', $node, '0')` | `(float)` → Decimal | 10 |
| 118 | `concepto` | `$xml->getValue('cac:Item/cbc:Description', $node)` | texto | 10 |

## RetentionParser

| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |
| --- | --- | --- | --- | --- |
| 53 | `serie` | `$idNum[0]` | texto | 164 |
| 54 | `correlativo` | `$idNum[1]` | texto | 188 |
| 55 | `fechaEmision` | `new DateTime($xml->getValue('cbc:IssueDate'))` | `new DateTime` | 154 |
| 56 | `company` | `$this->getCompany()` | texto | 152 |
| 57 | `proveedor` | `$this->getClient()` | texto | 24 |
| 58 | `regimen` | `$xml->getValue('sac:SUNATRetentionSystemCode')` | texto | 24 |
| 59 | `tasa` | `(float)$xml->getValue('sac:SUNATRetentionPercent', $root, '0')` | `(float)` → Decimal | 35 |
| 60 | `observacion` | `$xml->getValue('cbc:Note')` | texto | 42 |
| 61 | `impRetenido` | `(float)$xml->getValue('cbc:TotalInvoiceAmount', $root, '0')` | `(float)` → Decimal | 25 |
| 62 | `impPagado` | `(float)$xml->getValue('sac:SUNATTotalPaid', $root, '0')` | `(float)` → Decimal | 12 |
| 63 | `details` | `iterator_to_array($this->getDetails())` | texto | 152 |
| 74 | `ruc` | `$xml->getValue('cac:PartyIdentification/cbc:ID', $node)` | texto | 162 |
| 75 | `nombreComercial` | `$xml->getValue('cac:PartyName/cbc:Name', $node)` | texto | 134 |
| 76 | `razonSocial` | `$xml->getValue('cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 162 |
| 77 | `address` | `$this->getAddress($node)` | texto | 178 |
| 89 | `numDoc` | `$ident->nodeValue` | texto | 183 |
| 90 | `tipoDoc` | `$ident->getAttribute('schemeID')` | texto | 513 |
| 91 | `rznSocial` | `$xml->getValue('cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 157 |
| 92 | `address` | `$this->getAddress($node)` | texto | 178 |
| 107 | `direccion` | `$xml->getValue('cbc:StreetName', $address)` | texto | 209 |
| 108 | `departamento` | `$xml->getValue('cbc:CityName', $address)` | texto | 158 |
| 109 | `provincia` | `$xml->getValue('cbc:CountrySubentity', $address)` | texto | 158 |
| 110 | `distrito` | `$xml->getValue('cbc:District', $address)` | texto | 158 |
| 111 | `ubigueo` | `$xml->getValue('cbc:ID', $address)` | texto | 189 |
| 127 | `tipoDoc` | `$temp->getAttribute('schemeID')` | texto | 513 |
| 128 | `numDoc` | `$temp->nodeValue` | texto | 183 |
| 129 | `fechaEmision` | `new DateTime($xml->getValue('cbc:IssueDate', $node))` | `new DateTime` | 154 |
| 130 | `impTotal` | `(float)$mount->nodeValue` | `(float)` → Decimal | 26 |
| 131 | `moneda` | `$mount->getAttribute('currencyID')` | texto | 90 |
| 132 | `pagos` | `iterator_to_array($this->getPayments($node))` | texto | 13 |
| 136 | `impRetenido` | `0.00` | texto | 25 |
| 137 | `impPagar` | `0.00` | texto | 13 |
| 138 | `fechaRetencion` | `new DateTime()` | `new DateTime` | 13 |
| 145 | `impRetenido` | `(float)$xml->getValue('sac:SUNATRetentionAmount', $temp)` | `(float)` → Decimal | 25 |
| 146 | `fechaRetencion` | `new DateTime($xml->getValue('sac:SUNATRetentionDate', $temp))` | `new DateTime` | 13 |
| 147 | `impPagar` | `(float)$xml->getValue('sac:SUNATNetTotalPaid', $temp)` | `(float)` → Decimal | 13 |
| 152 | `monedaRef` | `$xml->getValue('cbc:SourceCurrencyCode', $cambio)` | texto | 20 |
| 153 | `monedaObj` | `$xml->getValue('cbc:TargetCurrencyCode', $cambio)` | texto | 20 |
| 154 | `factor` | `(float)$xml->getValue('cbc:CalculationRate', $cambio, '0')` | `(float)` → Decimal | 20 |
| 155 | `fecha` | `new DateTime($xml->getValue('cbc:Date', $cambio))` | `new DateTime` | 71 |
| 156 | `tipoCambio` | `$exc` | texto | 20 |
| 171 | `moneda` | `$temp->getAttribute('currencyID')` | texto | 90 |
| 172 | `importe` | `(float)$temp->nodeValue` | `(float)` → Decimal | 51 |
| 173 | `fecha` | `new DateTime($xml->getValue('cbc:PaidDate'))` | `new DateTime` | 71 |

## SummaryParser

| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |
| --- | --- | --- | --- | --- |
| 56 | `correlativo` | `$id[2]` | texto | 188 |
| 57 | `fecGeneracion` | `new DateTime($xml->getValue('cbc:ReferenceDate', $root))` | `new DateTime` | 34 |
| 58 | `fecResumen` | `new DateTime($xml->getValue('cbc:IssueDate', $root))` | `new DateTime` | 13 |
| 59 | `company` | `$this->getCompany()` | texto | 152 |
| 60 | `details` | `iterator_to_array($this->getDetails())` | texto | 152 |
| 71 | `ruc` | `$xml->getValue('cbc:CustomerAssignedAccountID', $node)` | texto | 162 |
| 72 | `nombreComercial` | `$xml->getValue('cac:Party/cac:PartyName/cbc:Name', $node)` | texto | 134 |
| 73 | `razonSocial` | `$xml->getValue('cac:Party/cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 162 |
| 85 | `tipoDoc` | `$xml->getValue('cbc:DocumentTypeCode', $node)` | texto | 513 |
| 86 | `serieNro` | `$xml->getValue('cbc:ID', $node)` | texto | 23 |
| 87 | `estado` | `trim($xml->getValue('cac:Status/cbc:ConditionCode', $node))` | `trim` | 23 |
| 88 | `clienteTipo` | `trim($xml->getValue('cac:AccountingCustomerParty/cbc:AdditionalAccountID', $node))` | `trim` | 23 |
| 89 | `clienteNro` | `trim($xml->getValue('cac:AccountingCustomerParty/cbc:CustomerAssignedAccountID', $node))` | `trim` | 23 |
| 90 | `total` | `(float)$xml->getValue('sac:TotalAmount', $node, '0')` | `(float)` → Decimal | 150 |
| 91 | `mtoOtrosCargos` | `(float)$xml->getValue('cac:AllowanceCharge/cbc:Amount', $node, '0')` | `(float)` → Decimal | 23 |
| 96 | `tipoDoc` | `trim($xml->getValue('cac:InvoiceDocumentReference/cbc:DocumentTypeCode', $ref))` | `trim` | 513 |
| 97 | `nroDoc` | `trim($xml->getValue('cac:InvoiceDocumentReference/cbc:ID', $ref))` | `trim` | 177 |
| 98 | `docReferencia` | `$doc` | texto | 12 |
| 118 | `codReg` | `trim($xml->getValue('sac:SUNATPerceptionSystemCode', $ref))` | `trim` | 83 |
| 119 | `tasa` | `(float)$xml->getValue('sac:SUNATPerceptionPercent', $ref, '0')` | `(float)` → Decimal | 35 |
| 120 | `mto` | `(float)$xml->getValue('sac:TotalInvoiceAmount', $ref, '0')` | `(float)` → Decimal | 83 |
| 121 | `mtoTotal` | `(float)$xml->getValue('sac:SUNATTotalCashed', $ref, '0')` | `(float)` → Decimal | 83 |
| 122 | `mtoBase` | `(float)$xml->getValue('sac:TaxableAmount', $ref, '0')` | `(float)` → Decimal | 83 |
| 124 | `percepcion` | `$perc` | texto | 11 |
| 137 | `mtoOperGravadas` | `$val` | texto | 82 |
| 140 | `mtoOperExoneradas` | `$val` | texto | 68 |
| 143 | `mtoOperInafectas` | `$val` | texto | 68 |
| 146 | `mtoOperGratuitas` | `$val` | texto | 67 |
| 161 | `mtoIGV` | `$val` | texto | 87 |
| 164 | `mtoISC` | `$val` | texto | 69 |
| 167 | `mtoOtrosTributos` | `$val` | texto | 87 |

## VoidedParser

| Línea | Propiedad | Expresión PHP | Conversión | Valores no nulos |
| --- | --- | --- | --- | --- |
| 53 | `correlativo` | `$id[2]` | texto | 188 |
| 54 | `fecGeneracion` | `new DateTime($xml->getValue('cbc:ReferenceDate', $root))` | `new DateTime` | 34 |
| 55 | `fecComunicacion` | `new DateTime($xml->getValue('cbc:IssueDate', $root))` | `new DateTime` | 21 |
| 56 | `company` | `$this->getCompany()` | texto | 152 |
| 57 | `details` | `iterator_to_array($this->getDetails())` | texto | 152 |
| 68 | `ruc` | `$xml->getValue('cbc:CustomerAssignedAccountID', $node)` | texto | 162 |
| 69 | `nombreComercial` | `$xml->getValue('cac:Party/cac:PartyName/cbc:Name', $node)` | texto | 134 |
| 70 | `razonSocial` | `$xml->getValue('cac:Party/cac:PartyLegalEntity/cbc:RegistrationName', $node)` | texto | 162 |
| 82 | `tipoDoc` | `$xml->getValue('cbc:DocumentTypeCode', $node)` | texto | 513 |
| 83 | `serie` | `$xml->getValue('sac:DocumentSerialID', $node)` | texto | 164 |
| 84 | `correlativo` | `$xml->getValue('sac:DocumentNumberID', $node)` | texto | 188 |
| 85 | `desMotivoBaja` | `$xml->getValue('sac:VoidReasonDescription', $node)` | texto | 36 |

Total de llamadas a setters: 323.
