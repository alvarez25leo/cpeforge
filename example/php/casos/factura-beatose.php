<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-beatose.php: F001-128 con errores
 * intencionales para probar BEAtOSE (solo se genera el XML).
 */

use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setFecVencimiento($c->fecha())
        ->setTipoOperacion('0101')
        ->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('128')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setObservacion('Factura para pruebas con BEAtOSE')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(200)
        ->setMtoIGV(36)
        ->setTotalImpuestos(36)
        ->setValorVenta(300) // Error intencional, valor correcto: 200
        ->setSubTotal(236)
        ->setMtoImpVenta(236)
    ;

    // Detalle gravado
    $item = new SaleDetail();
    $item->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setDescripcion('PROD 1')
        ->setCantidad(2)
        ->setMtoValorUnitario(100)
        ->setMtoValorVenta(200)
        ->setMtoBaseIgv(200)
        ->setPorcentajeIgv(18)
        ->setIgv(36)
        ->setTipAfeIgv('10') // Catalog: 07
        ->setTotalImpuestos(36)
        ->setMtoPrecioUnitario(100) // Error intencional, valor correcto: 118
    ;

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON TRESCIENTOS TREINTA Y SEIS CON OO/100 SOLES')
        ]);
};
