<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-descuento-nobase.php: FD01-1 con
 * descuentos (línea y global) que no afectan la base imponible.
 */

use Greenter\Model\Sale\Charge;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101') // Cat.51
        ->setTipoDoc('01')
        ->setSerie('FD01')
        ->setCorrelativo('1')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setDescuentos([
            (new Charge())
                ->setCodTipo('03') // Catalog. 53 (03: Descuento global que no afecta la Base Imponible)
                ->setMontoBase(3)
                ->setFactor(1)
                ->setMonto(3) // Mto Dscto
        ])
        ->setSumOtrosDescuentos(4.0 + 3.0) // suma descuentos que no afectan la base (dscto. detalles + dscto. global)
        ->setMtoOperGravadas(20)
        ->setMtoIGV(3.6)
        ->setTotalImpuestos(3.6)
        ->setValorVenta(20)
        ->setSubTotal(23.6)
        ->setMtoImpVenta(23.6 - 7.0); // SubTotal - SumOtrosDescuentos = 16.60

    $item = new SaleDetail();
    $item->setCodProducto('P004')
        ->setUnidad('NIU')
        ->setDescripcion('PRODUCTO 4')
        ->setCantidad(10)
        ->setDescuentos([
            (new Charge())
                ->setCodTipo('01') // Catalog. 53 (00: Descuento que no afecta la Base Imponible)
                ->setMontoBase(20) // cantidad * valor unitario
                ->setFactor(0.2) // 20% descuento
                ->setMonto(4)
        ])
        ->setMtoValorUnitario(2)
        ->setMtoValorVenta(20) // cantidad * valor unitario
        ->setMtoBaseIgv(20)
        ->setPorcentajeIgv(18) // 18%
        ->setIgv(3.6)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(3.6) // IGV + ISC + OTH + ICBPER
        ->setMtoPrecioUnitario(1.96); // (Valor venta + Total Impuestos - Descuentos que no afectan la base) / Cantidad

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON SETENTA Y SIETE CON 88/100 SOLES')
        ]);
};
