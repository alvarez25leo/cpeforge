<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/factura-exportacion.php: F001-125, exportación en USD a cliente extranjero.
 */

use Greenter\Model\Client\Client;
use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $invoice = new Invoice();
    $invoice
        ->setUblVersion('2.1')
        ->setTipoOperacion('0200') // Tipo de operación: exportación
        ->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('125')
        ->setFechaEmision($c->fecha())
        ->setFormaPago(new FormaPagoContado())
        ->setTipoMoneda('USD')
        ->setCompany(empresa())
        ->setClient((new Client()) // Cliente: extranjero o sin documentos
            ->setTipoDoc('0')
            ->setNumDoc('-')
            ->setRznSocial('EXTRANJERO 1')
        )
        ->setMtoOperExportacion(100)
        ->setMtoIGV(0)
        ->setTotalImpuestos(0)
        ->setValorVenta(100)
        ->setSubTotal(100)
        ->setMtoImpVenta(100);

    $item = new SaleDetail();
    $item->setCodProducto('P001')
        ->setCodProdSunat('44121618') // Código de producto SUNAT, requerido.
        ->setUnidad('KG')
        ->setDescripcion('PROD 1')
        ->setCantidad(2)
        ->setMtoValorUnitario(50)
        ->setMtoValorVenta(100)
        ->setMtoBaseIgv(100)
        ->setPorcentajeIgv(0)
        ->setIgv(0)
        ->setTipAfeIgv('40')
        ->setTotalImpuestos(0)
        ->setMtoPrecioUnitario(50)
    ;

    return $invoice->setDetails([$item])
        ->setLegends([
            (new Legend())
                ->setCode('1000')
                ->setValue('SON CIEN CON OO/100 SOLES')
        ]);
};
