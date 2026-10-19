<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-icbper-gratuita.php: F001-130, gravada con bolsa plástica gratuita (ICBPER).
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
        ->setCorrelativo('130')
        ->setCompany(empresa())
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('PEN')
        ->setClient(cliente())
        ->setMtoOperGravadas(200)
        ->setMtoOperGratuitas(0.10)
        ->setMtoIGVGratuitas(0.02)
        ->setMtoIGV(36)
        ->setIcbper(0.40)
        ->setTotalImpuestos(36.40)
        ->setValorVenta(200)
        ->setSubTotal(236.40)
        ->setMtoImpVenta(236.40);

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
        ->setCantidad(2)
        ->setDescripcion('BOLSA DE PLASTICO')
        ->setMtoValorUnitario(0.00)
        ->setMtoValorGratuito(0.05)
        ->setMtoValorVenta(0.10)
        ->setTipAfeIgv('13') // catalog: 07, código de afectación gratuito
        ->setMtoBaseIgv(0.10)
        ->setPorcentajeIgv(18.0)
        ->setIgv(0.02)
        ->setFactorIcbper(0.20) // Factor ICBPER del año actual
        ->setIcbper(0.40) // (cantidad)*(factor ICBPER)
        ->setTotalImpuestos(0.42)
    ;

    $legend = new Legend();
    $legend->setCode('1000')
        ->setValue('SON DOSCIENTOS TREINTA Y SIETE CON 00/100 SOLES');

    return $invoice->setDetails([$detail, $detailBolsa])
        ->setLegends([$legend]);
};
