<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-contingencia.php: 0001-1, factura de
 * contingencia (serie numérica).
 */

use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101')
        ->setTipoDoc('01')
        ->setSerie('0001')
        ->setCorrelativo('1')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setClient(cliente())
        ->setMtoOperGravadas(200)
        ->setMtoIGV(36)
        ->setTotalImpuestos(36)
        ->setValorVenta(200)
        ->setSubTotal(236)
        ->setMtoImpVenta(236)
        ->setCompany(empresa());

    $item1 = new SaleDetail();
    $item1->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setDescripcion('PROD 1')
        ->setCantidad(2)
        ->setMtoValorUnitario(100)
        ->setMtoValorVenta(200)
        ->setMtoBaseIgv(200)
        ->setPorcentajeIgv(18)
        ->setIgv(36)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(36)
        ->setMtoPrecioUnitario(118)
    ;

    return $invoice->setDetails([$item1])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON DOSCIENTOS TREINTA Y SEIS CON OO/100 SOLES')
        ]);
};
