<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura.php: F001-123, gravada y exonerada.
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
        ->setCorrelativo('123')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(200)
        ->setMtoOperExoneradas(100)
        ->setMtoIGV(36)
        ->setTotalImpuestos(36)
        ->setValorVenta(300)
        ->setSubTotal(336)
        ->setMtoImpVenta(336)
    ;

    $items = [];

    // Detalle gravado
    $items[] = (new SaleDetail())
        ->setCodProducto('P001')
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
        ->setMtoPrecioUnitario(118)
    ;

    // Detalle Exonerado
    $items[] = (new SaleDetail())
        ->setCodProducto('P002')
        ->setUnidad('KG')
        ->setDescripcion('PROD 2')
        ->setCantidad(2)
        ->setMtoValorUnitario(50)
        ->setMtoValorVenta(100)
        ->setMtoBaseIgv(100)
        ->setPorcentajeIgv(0)
        ->setIgv(0)
        ->setTipAfeIgv('20') // Catalog: 07
        ->setTotalImpuestos(0)
        ->setMtoPrecioUnitario(50)
    ;

    return $invoice->setDetails($items)
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON TRESCIENTOS TREINTA Y SEIS CON OO/100 SOLES')
        ]);
};
