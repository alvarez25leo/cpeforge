<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-rel-guia-remision.php: F001-223 con guía
 * de remisión relacionada.
 */

use Greenter\Model\Sale\Document;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $guiaRemision = (new Document())
        ->setTipoDoc('09') // Guia de Remision remitente: 09, catalogo 01
        ->setNroDoc('T001-2'); // Serie y correlativo de la guia de remision

    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setFecVencimiento($c->fecha())
        ->setTipoOperacion('0101')
        ->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('223')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setGuias([
            $guiaRemision // Incluir guia remision.
        ])
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(200)
        ->setMtoIGV(36)
        ->setTotalImpuestos(36)
        ->setValorVenta(200)
        ->setSubTotal(236)
        ->setMtoImpVenta(236);

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
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(36)
        ->setMtoPrecioUnitario(118);

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON TRESCIENTOS TREINTA Y SEIS CON OO/100 SOLES')
        ]);
};
