<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-retencion.php: F001-168 con retención del
 * 3 % (descuento código 62).
 */

use Greenter\Model\Sale\Charge;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    // Venta
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101')
        ->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('168')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setClient(cliente())
        ->setCompany(empresa())
        ->setDescuentos([
            (new Charge())
                ->setCodTipo('62') // Catalog. 53
                ->setMontoBase(236)
                ->setFactor(0.03) // 3%
                ->setMonto(7.08)
        ])
        ->setMtoOperGravadas(200)
        ->setMtoIGV(36)
        ->setTotalImpuestos(36)
        ->setValorVenta(200)
        ->setSubTotal(236)
        ->setMtoImpVenta(236);

    $detail = new SaleDetail();
    $detail->setCodProducto('P001')
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

    return $invoice->setDetails([$detail])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON DOSCIENTOS TREINTA Y SEIS CON OO/100 SOLES'),
        ]);
};
