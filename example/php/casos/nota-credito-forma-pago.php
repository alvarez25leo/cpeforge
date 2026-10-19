<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/nota-credito-forma-pago.php: FF01-124, ajuste de
 * montos y fechas de pago con forma de pago al crédito y una cuota.
 *
 * SUNAT exige hoy que una nota con motivo 13 tenga todos los importes en cero
 * (error 3315); el demo de 2023 usaba 236.00. La cuota conserva el monto del
 * nuevo cronograma de pago.
 */

use Greenter\Model\Sale\Cuota;
use Greenter\Model\Sale\Document;
use Greenter\Model\Sale\FormaPagos\FormaPagoCredito;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\Note;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Note {
    $note = new Note();
    $note
        ->setUblVersion('2.1')
        ->setTipoDoc('07')
        ->setSerie('FF01')
        ->setCorrelativo('124')
        ->setFechaEmision($c->fecha())
        // Forma de Pago
        ->setFormaPago(new FormaPagoCredito(236))
        ->setCuotas([
            (new Cuota())
            ->setMonto(236)
            ->setFechaPago($c->dias(5))
        ])
        ->setTipDocAfectado('01') // Tipo Doc: Factura
        ->setNumDocfectado('F001-111') // Factura: Serie-Correlativo
        ->setCodMotivo('13') // Catalogo. 09: Ajustes – montos y/o fechas de pago
        ->setDesMotivo('DEVOLUCION POR ITEM')
        ->setTipoMoneda('PEN')
        ->setGuias([/* Guias (Opcional) */
            (new Document())
            ->setTipoDoc('09')
            ->setNroDoc('0001-213')
        ])
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setMtoOperGravadas(0)
        ->setMtoIGV(0)
        ->setTotalImpuestos(0)
        ->setMtoImpVenta(0)
        ;

    $detail1 = new SaleDetail();
    $detail1
        ->setCodProducto('C023')
        ->setUnidad('NIU')
        ->setCantidad(2)
        ->setDescripcion('PROD 1')
        ->setMtoBaseIgv(0)
        ->setPorcentajeIgv(18.00)
        ->setIgv(0)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(0)
        ->setMtoValorVenta(0)
        ->setMtoValorUnitario(0)
        ->setMtoPrecioUnitario(0);

    $detail2 = new SaleDetail();
    $detail2
        ->setCodProducto('C02')
        ->setUnidad('NIU')
        ->setCantidad(2)
        ->setDescripcion('PROD 2')
        ->setMtoBaseIgv(0)
        ->setPorcentajeIgv(18.00)
        ->setIgv(0)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(0)
        ->setMtoValorVenta(0)
        ->setMtoValorUnitario(0)
        ->setMtoPrecioUnitario(0);

    $legend = new Legend();
    $legend->setCode('1000')
        ->setValue('SON CERO CON 00/100 SOLES');

    return $note->setDetails([$detail1, $detail2])
        ->setLegends([$legend]);
};
