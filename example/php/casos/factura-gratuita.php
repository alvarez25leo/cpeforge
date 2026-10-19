<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-gratuita.php: F001-131, transferencia gratuita.
 */

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
        ->setCorrelativo('131')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setClient(cliente())
        ->setCompany(empresa())
        ->setMtoOperGratuitas(200)
        ->setMtoIGVGratuitas(36)
        ->setTotalImpuestos(0)
        ->setValorVenta(0)
        ->setSubTotal(0)
        ->setMtoImpVenta(0);

    $detail = new SaleDetail();
    $detail->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setDescripcion('PROD 1')
        ->setCantidad(2)
        ->setMtoValorUnitario(0)
        ->setMtoValorGratuito(100)
        ->setMtoValorVenta(200)
        ->setMtoBaseIgv(200)
        ->setPorcentajeIgv(18)
        ->setIgv(36)
        ->setTipAfeIgv('11')
        ->setTotalImpuestos(36)
        ->setMtoPrecioUnitario(0)
    ;

    return $invoice->setDetails([$detail])
        ->setLegends([
            (new Legend())
                ->setCode('1002')
                ->setValue('TRANSFERENCIA GRATUITA DE UN BIEN Y/O SERVICIO PRESTADO GRATUITAMENTE')
        ]);
};
