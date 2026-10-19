<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-descuento-linea.php: FE01-1 con
 * descuento por línea que afecta la base imponible.
 */

use Greenter\Model\Sale\Charge;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    // Factura con descuento por linea (que afecta la base imponible).
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101') // Cat.51
        ->setTipoDoc('01')
        ->setSerie('FE01')
        ->setCorrelativo('1')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(66) // suma de v. venta (items).
        ->setMtoIGV(11.88)
        ->setTotalImpuestos(11.88)
        ->setValorVenta(66)
        ->setSubTotal(77.88)
        ->setMtoImpVenta(77.88);

    $itemDescuento = new SaleDetail();
    $itemDescuento->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setDescripcion('Cuadernos')
        ->setCantidad(10)
        ->setDescuentos([
            (new Charge())
                ->setCodTipo('00') // Catalog. 53 (00: Descuento que afecta la Base Imponible)
                ->setMontoBase(20) // cantidad * valor unitario
                ->setFactor(0.2) // 20% descuento
                ->setMonto(4)
        ])
        ->setMtoValorUnitario(2)
        ->setMtoValorVenta(16) // cantidad * valor unitario - descuento (que afecta la base)
        ->setMtoBaseIgv(16)
        ->setPorcentajeIgv(18)
        ->setIgv(2.88)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(2.88)
        ->setMtoPrecioUnitario(1.89); // (Valor venta + Total Impuestos) / Cantidad

    $item2 = new SaleDetail();
    $item2->setCodProducto('P002')
        ->setUnidad('NIU')
        ->setDescripcion('Radio')
        ->setCantidad(1)
        ->setMtoValorUnitario(50)
        ->setMtoValorVenta(50)
        ->setMtoBaseIgv(50)
        ->setPorcentajeIgv(18)
        ->setIgv(9)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(9)
        ->setMtoPrecioUnitario(59);

    return $invoice->setDetails([$itemDescuento, $item2])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON SETENTA Y SIETE CON 88/100 SOLES')
        ]);
};
