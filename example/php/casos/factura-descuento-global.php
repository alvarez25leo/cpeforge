<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-descuento-global.php: F001-84 con
 * descuento global que afecta la base imponible.
 */

use Greenter\Model\Sale\Charge;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    // Factura con descuento global (que afecta la base imponible).
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setFecVencimiento($c->fecha())
        ->setTipoOperacion('0101')
        ->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('84')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setDescuentos([
            (new Charge())
                ->setCodTipo('02') // Catalog. 53
                ->setMontoBase(3)
                ->setFactor(1)
                ->setMonto(3)
        ])
        ->setMtoOperGravadas(67) // suma de v. venta (items) - descuento global.
        ->setMtoIGV(12.06)
        ->setTotalImpuestos(12.06)
        ->setValorVenta(67)
        ->setSubTotal(79.06)
        ->setMtoImpVenta(79.06)
    ;

    $item1 = new SaleDetail();
    $item1->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setDescripcion('Cuadernos')
        ->setCantidad(10)
        ->setMtoValorUnitario(2)
        ->setMtoValorVenta(20)
        ->setMtoBaseIgv(20)
        ->setPorcentajeIgv(18)
        ->setIgv(3.6)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(3.6)
        ->setMtoPrecioUnitario(2.36)
    ;

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
        ->setMtoPrecioUnitario(59)
    ;

    return $invoice->setDetails([$item1, $item2])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON SETENTA Y NUEVE CON O6/100 SOLES')
        ]);
};
