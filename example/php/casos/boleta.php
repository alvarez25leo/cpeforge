<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/boleta.php: B001-1 a persona con DNI.
 */

use Greenter\Model\Client\Client;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    // Cliente
    $client = new Client();
    $client->setTipoDoc('1')
        ->setNumDoc('20203030')
        ->setRznSocial('PERSON 1');

    // Venta
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101')
        ->setTipoDoc('03')
        ->setSerie('B001')
        ->setCorrelativo('1')
        ->setFechaEmision($c->fecha())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient($client)
        ->setMtoOperGravadas(100)
        ->setMtoIGV(18)
        ->setTotalImpuestos(18)
        ->setValorVenta(100)
        ->setSubTotal(118)
        ->setMtoImpVenta(118)
        ;

    $item1 = new SaleDetail();
    $item1->setCodProducto('C023')
        ->setUnidad('NIU')
        ->setCantidad(2)
        ->setDescripcion('PROD 1')
        ->setMtoBaseIgv(100)
        ->setPorcentajeIgv(18)
        ->setIgv(18)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(18)
        ->setMtoValorVenta(100)
        ->setMtoValorUnitario(50)
        ->setMtoPrecioUnitario(59);

    $legend = new Legend();
    $legend->setCode('1000')
        ->setValue('SON CIENTO DIECIOCHO CON 00/100 SOLES');

    return $invoice->setDetails([$item1])
        ->setLegends([$legend]);
};
