<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-ivap.php: F001-226, venta sujeta al IVAP.
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
        ->setCorrelativo('226')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoBaseIvap(90000.00) // Base IVAP
        ->setMtoIvap(3600.00) // Suma IVAP
        ->setTotalImpuestos(3600.00)
        ->setValorVenta(90000.00)
        ->setSubTotal(93600.00)
        ->setMtoImpVenta(93600.00)
    ;

    $item = new SaleDetail();
    $item->setCodProducto('A001')
        ->setUnidad('SA') // Codigo unidad de saco
        ->setDescripcion('SACOS DE ARROZ')
        ->setCantidad(900.00)
        ->setMtoValorUnitario(100.00)
        ->setMtoValorVenta(90000.00)
        ->setMtoBaseIgv(90000.00)
        ->setPorcentajeIgv(4) // Tasa IVAP
        ->setIgv(3600.00)
        ->setTipAfeIgv('17') // Tipo Afectacion IVAP
        ->setTotalImpuestos(3600.00)
        ->setMtoPrecioUnitario(104.00)
    ;

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON TRESCIENTOS TREINTA Y SEIS CON OO/100 SOLES'),
            (new Legend())
                ->setCode('2007') // Leyenda IVAP
                ->setValue("Operación sujeta al IVAP"),
        ]);
};
