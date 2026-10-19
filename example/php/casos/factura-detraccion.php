<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-detraccion.php: F001-127, operación sujeta a detracción.
 */

use Greenter\Model\Sale\Detraction;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('1001') // Catálogo 51: Detracción
        ->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('127')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(800)
        ->setMtoIGV(144)
        ->setTotalImpuestos(144)
        ->setValorVenta(800)
        ->setSubTotal(944)
        ->setMtoImpVenta(944)
        ->setDetraccion(
        // MONEDA SIEMPRE EN SOLES
            (new Detraction())
                // Carnes y despojos comestibles
                ->setCodBienDetraccion('014') // catalog. 54
                // Depósito en cuenta
                ->setCodMedioPago('001') // catalog. 59
                ->setCtaBanco('0004-3342343243')
                ->setPercent(4.00)
                ->setMount(37.76)
        );

    $detail = new SaleDetail();
    $detail->setCodProducto('P001')
        ->setUnidad('ZZ')
        ->setDescripcion('PROD 1')
        ->setCantidad(4)
        ->setMtoValorUnitario(200)
        ->setMtoValorVenta(800)
        ->setMtoBaseIgv(800)
        ->setPorcentajeIgv(18)
        ->setIgv(144)
        ->setTipAfeIgv('10') // Catálogo 07 - Gravado
        ->setTotalImpuestos(144)
        ->setMtoPrecioUnitario(236)
    ;

    return $invoice->setDetails([$detail])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON NOVECIENTOS CUARENTA Y CUATRO CON OO/100 SOLES'),
            (new Legend())
                ->setCode('2006')
                ->setValue('Operación sujeta a detracción')
        ]);
};
