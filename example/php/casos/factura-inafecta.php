<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-inafecta.php: F001-137, operación inafecta.
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
        ->setSerie('F001')
        ->setCorrelativo('137')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperInafectas(100)
        ->setMtoIGV(0)
        ->setTotalImpuestos(0)
        ->setValorVenta(100)
        ->setSubTotal(100)
        ->setMtoImpVenta(100);

    $item = new SaleDetail();
    $item->setCodProducto('P001')
        ->setUnidad('KG')
        ->setDescripcion('PROD 1')
        ->setCantidad(2)
        ->setMtoValorUnitario(50)
        ->setMtoValorVenta(100)
        ->setMtoBaseIgv(100)
        ->setPorcentajeIgv(0)
        ->setIgv(0)
        ->setTipAfeIgv('30')
        ->setTotalImpuestos(0)
        ->setMtoPrecioUnitario(50)
    ;

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON CIEN CON OO/100 SOLES')
        ]);
};
