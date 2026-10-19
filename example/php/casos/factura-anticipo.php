<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-anticipo.php: F001-143 con deducción de
 * un anticipo (F001-111).
 */

use Greenter\Model\Sale\Charge;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\Prepayment;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101')
        ->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('143')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setClient(cliente())
        ->setCompany(empresa())
        ->setDescuentos([(
            new Charge())
            ->setCodTipo('04')
            ->setFactor(1)
            ->setMonto(100) // anticipo
            ->setMontoBase(100)
        ])
        ->setMtoOperGravadas(100) // Sumatoria de Valor Venta (detalles) menos descuentos globales (anticipo): 200 - 100
        ->setMtoIGV(18)
        ->setValorVenta(200) // sumatoria de valor venta (detalle)
        ->setTotalImpuestos(18)
        ->setSubTotal(236) // ValorVenta + (sumatoria de valor venta detalle) * 18% (IGV)
        ->setMtoImpVenta(136) // subTotal - Anticipos: 236 - 100
        ->setAnticipos([
            (new Prepayment())
                ->setTipoDocRel('02') // catalog. 12
                ->setNroDocRel('F001-111')
                ->setTotal(100)
        ])
        ->setTotalAnticipos(100);

    $detail = new SaleDetail();
    $detail->setCodProducto('P001')
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
        ->setMtoPrecioUnitario(118)
    ;

    return $invoice->setDetails([$detail])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON DOSCIENTOS TREINTA Y SEIS CON OO/100 SOLES')
        ]);
};
