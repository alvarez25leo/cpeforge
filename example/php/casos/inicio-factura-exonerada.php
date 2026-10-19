<?php

declare(strict_types=1);

/*
 * F001-2: factura exonerada (https://greenter.dev/examples/exonerada/).
 */

use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $detail = (new SaleDetail())
        ->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setDescripcion('PROD 1')
        ->setCantidad(2)
        ->setMtoValorUnitario(100)
        ->setMtoValorVenta(200)
        ->setMtoBaseIgv(200)
        ->setPorcentajeIgv(18)
        ->setIgv(0)
        ->setTipAfeIgv('20') // Exonerado - Operación onerosa
        ->setTotalImpuestos(0)
        ->setMtoPrecioUnitario(100);

    return inicio_invoice('01', 'F001', '2', $c->fecha(), inicio_emisor($c->ruc), inicio_cliente_empresa())
        ->setFecVencimiento(clone $c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setMtoOperExoneradas(200)
        ->setMtoIGV(0)
        ->setTotalImpuestos(0)
        ->setValorVenta(200)
        ->setSubTotal(200)
        ->setMtoImpVenta(200)
        ->setDetails([$detail])
        ->setLegends([
            (new Legend())->setCode('1000')->setValue('SON DOSCIENTOS CON 00/100 SOLES'),
        ]);
};
