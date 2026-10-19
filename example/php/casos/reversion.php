<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/reversion.php: RR de una retención y una
 * percepción.
 */

use Greenter\Model\Voided\Reversion;
use Greenter\Model\Voided\VoidedDetail;

return static function (Contexto $c): Reversion {
    $item1 = new VoidedDetail();
    $item1->setTipoDoc('20')
        ->setSerie('R001')
        ->setCorrelativo('122')
        ->setDesMotivoBaja('ERROR DE SISTEMA');

    $item2 = new VoidedDetail();
    $item2->setTipoDoc('40')
        ->setSerie('P001')
        ->setCorrelativo('111')
        ->setDesMotivoBaja('ERROR DE RUC');

    $reversion = new Reversion();
    $reversion->setCorrelativo('001')
        ->setFecGeneracion($c->dias(-3))
        ->setFecComunicacion($c->dias(-1))
        ->setCompany(empresa())
        ->setDetails([$item1, $item2]);

    return $reversion;
};
