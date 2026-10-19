<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/nota-debito.php: FF01-123 por aumento en el valor.
 */

use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\Note;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Note {
    $note = new Note();
    $note
        ->setUblVersion('2.1')
        ->setTipDocAfectado('01')
        ->setNumDocfectado('F001-111')
        ->setCodMotivo('02')
        ->setDesMotivo('AUMENTO EN EL VALOR')
        ->setTipoDoc('08')
        ->setSerie('FF01')
        ->setFechaEmision($c->fecha())
        ->setCorrelativo('123')
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(200)
        ->setMtoIGV(36)
        ->setTotalImpuestos(36)
        ->setMtoImpVenta(236)
        ;

    $detail1 = new SaleDetail();
    $detail1
        ->setCodProducto('C023')
        ->setUnidad('NIU')
        ->setCantidad(2)
        ->setDescripcion('PROD 1')
        ->setMtoBaseIgv(100)
        ->setPorcentajeIgv(18.00)
        ->setIgv(18)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(18)
        ->setMtoValorVenta(100)
        ->setMtoValorUnitario(50)
        ->setMtoPrecioUnitario(59);

    $detail2 = new SaleDetail();
    $detail2
        ->setCodProducto('C02')
        ->setUnidad('NIU')
        ->setCantidad(2)
        ->setDescripcion('PROD 2')
        ->setMtoBaseIgv(100)
        ->setPorcentajeIgv(18.00)
        ->setIgv(18)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(18)
        ->setMtoValorVenta(100)
        ->setMtoValorUnitario(50)
        ->setMtoPrecioUnitario(59);

    $legend = new Legend();
    $legend->setCode('1000')
        ->setValue('SON DOSCIENTOS TREINTA Y SEIS CON 00/100 SOLES');

    return $note->setDetails([$detail1, $detail2])
        ->setLegends([$legend]);
};
