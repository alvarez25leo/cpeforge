<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-isc.php: F001-227, gravada con ISC (sistema al valor).
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
        // El demo usa '127', que choca con factura-detraccion; aquí se usa '227'.
        ->setCorrelativo('227')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(200)
        ->setMtoIGV(42.12)
        ->setMtoBaseIsc(200) // Sumatoria MtoBaseISC de los detalles
        ->setMtoISC(34)
        ->setTotalImpuestos(76.12)
        ->setValorVenta(200)
        ->setSubTotal(276.12)
        ->setMtoImpVenta(276.12)
    ;

    // Detalle con ISC
    $item = new SaleDetail();
    $item->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setDescripcion('PROD 1')
        ->setCantidad(2)
        ->setMtoValorUnitario(100)
        ->setMtoValorVenta(200)
        ->setMtoBaseIsc(200)
        ->setTipSisIsc('01') // Catalog 08: Sistema al Valor
        ->setPorcentajeIsc(17.00) // 17%
        ->setIsc(34) // 200 * 0.17 (17%)
        ->setMtoBaseIgv(234) // ValorVenta + ISC
        ->setPorcentajeIgv(18)
        ->setIgv(42.12) // MtoBaseIGV * 18%
        ->setTipAfeIgv('10') // Catalog: 07
        ->setTotalImpuestos(76.12)
        ->setMtoPrecioUnitario(138.06) // (ValorVenta + TotalImpuestos) / Cantidad
    ;

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON DOSCIENTOS SETENTA Y SEIS CON 12/100 SOLES')
        ]);
};
