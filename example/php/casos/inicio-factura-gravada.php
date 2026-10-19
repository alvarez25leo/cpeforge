<?php

declare(strict_types=1);

/*
 * F001-1: factura gravada al contado (https://greenter.dev/starter/).
 */

use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;

return static function (Contexto $c): Invoice {
    return inicio_invoice('01', 'F001', '1', $c->fecha(), inicio_emisor($c->ruc), inicio_cliente_empresa())
        ->setFormaPago(new FormaPagoContado())
        ->setMtoOperGravadas(100.00)
        ->setMtoIGV(18.00)
        ->setTotalImpuestos(18.00)
        ->setValorVenta(100.00)
        ->setSubTotal(118.00)
        ->setMtoImpVenta(118.00)
        ->setDetails([
            inicio_linea_gravada('P001', 'PRODUCTO 1', 2, 50.00, 100.00, 18.00, 59.00),
        ])
        ->setLegends([
            (new Legend())
                ->setCode('1000') // Monto en letras - Catálogo 52
                ->setValue('SON CIENTO DIECIOCHO CON 00/100 SOLES'),
        ]);
};
