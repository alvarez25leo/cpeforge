<?php

declare(strict_types=1);

/*
 * F001-150 con un cargo global que no afecta la base imponible: recargo al
 * consumo del 10 % (catálogo 53, código 46).
 */

use Greenter\Model\Sale\Charge;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    // El recargo se suma al total a pagar (ChargeTotalAmount), sin IGV.
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101')
        ->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('150')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setCompany(empresa())
        ->setClient(cliente())
        ->setCargos([
            (new Charge())
                ->setCodTipo('46') // Catálogo 53: recargo al consumo y/o propinas.
                ->setFactor(0.10)
                ->setMonto(10)
                ->setMontoBase(100)
        ])
        ->setMtoOperGravadas(100)
        ->setMtoIGV(18)
        ->setTotalImpuestos(18)
        ->setSumOtrosCargos(10)
        ->setValorVenta(100)
        ->setSubTotal(118)
        ->setMtoImpVenta(128) // 118 + 10 de recargo.
    ;

    $item = new SaleDetail();
    $item->setCodProducto('P001')
        ->setUnidad('NIU')
        ->setDescripcion('Menú ejecutivo')
        ->setCantidad(2)
        ->setMtoValorUnitario(50)
        ->setMtoValorVenta(100)
        ->setMtoBaseIgv(100)
        ->setPorcentajeIgv(18)
        ->setIgv(18)
        ->setTipAfeIgv('10')
        ->setTotalImpuestos(18)
        ->setMtoPrecioUnitario(59)
    ;

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON CIENTO VEINTIOCHO CON 00/100 SOLES')
        ]);
};
