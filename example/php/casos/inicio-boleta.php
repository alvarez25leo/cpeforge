<?php

declare(strict_types=1);

/*
 * B001-3: boleta a persona natural con líneas gravadas y bolsa (ICBPER).
 * Total 96.58.
 */

use Greenter\Model\Sale\FormaPagos\FormaPagoContado;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;

return static function (Contexto $c): Invoice {
    return inicio_invoice('03', 'B001', '3', $c->fecha(), inicio_emisor($c->ruc), inicio_cliente_persona())
        ->setFormaPago(new FormaPagoContado())
        ->setMtoOperGravadas(81.00)
        ->setMtoIGV(14.58)
        ->setIcbper(1.00)
        ->setTotalImpuestos(15.58)
        ->setValorVenta(81.00)
        ->setSubTotal(96.58)
        ->setMtoImpVenta(96.58)
        ->setDetails([
            inicio_linea_gravada('C023', 'CUADERNO A4', 3, 10.00, 30.00, 5.40, 11.80),
            inicio_linea_gravada('C024', 'LAPICERO AZUL X 10', 2, 25.00, 50.00, 9.00, 29.50),
            inicio_linea_bolsa(2, 1.00, 0.18, 1.00),
        ])
        ->setLegends([
            (new Legend())->setCode('1000')->setValue('SON NOVENTA Y SEIS CON 58/100 SOLES'),
        ]);
};
