<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-credito.php: F001-132 a crédito en dos
 * cuotas (+7 y +14 días).
 */

use Greenter\Model\Sale\Cuota;
use Greenter\Model\Sale\FormaPagos\FormaPagoCredito;
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
        ->setCorrelativo('132')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoCredito(236))
        ->setCuotas([
            (new Cuota())
                ->setMonto(100)
                ->setFechaPago($c->dias(7)),
            (new Cuota())
                ->setMonto(136)
                ->setFechaPago($c->dias(14))
        ])
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(200)
        ->setMtoIGV(36)
        ->setTotalImpuestos(36)
        ->setValorVenta(200)
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
        ->setMtoPrecioUnitario(118)
    ;

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON DOSCIENTOS TREINTA Y SEIS CON OO/100 SOLES')
        ]);
};
