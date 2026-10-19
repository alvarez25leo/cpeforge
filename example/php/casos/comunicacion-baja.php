<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/comunicacion-baja.php: RA de una factura y una
 * nota de crédito.
 */

use Greenter\Model\Voided\Voided;
use Greenter\Model\Voided\VoidedDetail;

return static function (Contexto $c): Voided {
    $detail1 = new VoidedDetail();
    $detail1->setTipoDoc('01')
        ->setSerie('F001')
        ->setCorrelativo('02132132')
        ->setDesMotivoBaja('ERROR EN CÁLCULOS');

    $detail2 = new VoidedDetail();
    $detail2->setTipoDoc('07')
        ->setSerie('FC01')
        ->setCorrelativo('222')
        ->setDesMotivoBaja('ERROR DE RUC');

    $voided = new Voided();

    return $voided->setCorrelativo('00111')
        // Fecha Generacion menor que Fecha comunicacion
        ->setFecGeneracion($c->dias(-3))
        ->setFecComunicacion($c->dias(-1))
        ->setCompany(empresa())
        ->setDetails([$detail1, $detail2]);
};
