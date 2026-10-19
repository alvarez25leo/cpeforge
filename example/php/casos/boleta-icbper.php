<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/boleta-icbper.php: B001-2 con bolsa de plástico
 * (ICBPER) y redondeo.
 */

use Greenter\Model\Client\Client;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    // Cliente (el demo lo crea, pero la boleta usa el cliente compartido)
    $client = new Client();
    $client->setTipoDoc('1')
        ->setNumDoc('74002155')
        ->setRznSocial('JUAN BRAIN');

    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101')
        ->setTipoDoc('03')
        ->setSerie('B001')
        ->setCorrelativo('2')
        ->setCompany(empresa())
        ->setFechaEmision($c->fecha())
        ->setTipoMoneda('PEN')
        ->setClient(cliente())
        ->setMtoOperGravadas(200.20)
        ->setMtoIGV(36.04)
        ->setIcbper(0.80)
        ->setTotalImpuestos(36.84)
        ->setValorVenta(200.20)
        ->setSubTotal(237.04)
        ->setRedondeo(0.04)
        ->setMtoImpVenta(237);

    $detail = new SaleDetail();
    $detail
        ->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setCantidad(2)
        ->setDescripcion('PRODUCTO 1')
        ->setMtoBaseIgv(200.00)
        ->setPorcentajeIgv(18.0)
        ->setIgv(36)
        ->setTotalImpuestos(36)
        ->setTipAfeIgv('10')
        ->setMtoValorVenta(200)
        ->setMtoValorUnitario(100)
        ->setMtoPrecioUnitario(118);

    $detailBolsa = new SaleDetail();
    $detailBolsa
        ->setCodProducto('P002')
        ->setUnidad('NIU')
        ->setCantidad(4)
        ->setDescripcion('BOLSA DE PLASTICO')
        ->setMtoValorUnitario(0.05)
        ->setMtoPrecioUnitario(0.059)
        ->setMtoValorVenta(0.20)
        ->setTipAfeIgv('10')
        ->setMtoBaseIgv(0.20)
        ->setPorcentajeIgv(18.0)
        ->setIgv(0.04)
        ->setIcbper(0.80) // (cantidad)*(factor ICBPER)
        ->setFactorIcbper(0.20)
        ->setTotalImpuestos(0.84)
    ;

    $legend = new Legend();
    $legend->setCode('1000')
        ->setValue('SON DOSCIENTOS TREINTA Y SIETE CON 00/100 SOLES');

    return $invoice->setDetails([$detail, $detailBolsa])
        ->setLegends([$legend]);
};
