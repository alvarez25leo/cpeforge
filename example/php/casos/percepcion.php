<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/percepcion.php: P001-123.
 */

use Greenter\Model\Perception\Perception;
use Greenter\Model\Perception\PerceptionDetail;
use Greenter\Model\Retention\Exchange;
use Greenter\Model\Retention\Payment;

return static function (Contexto $c): Perception {
    $perception = new Perception();
    $perception
        ->setSerie('P001')
        ->setCorrelativo('123')
        ->setFechaEmision($c->fecha())
        ->setObservacion('NOTA PRUEBA />')
        ->setCompany(empresa())
        ->setProveedor(cliente())
        ->setImpPercibido(10)
        ->setImpCobrado(210)
        ->setRegimen('01')
        ->setTasa(2);

    $pay = new Payment();
    $pay->setMoneda('PEN')
        ->setFecha($c->fecha())
        ->setImporte(100);

    $cambio = new Exchange();
    $cambio->setFecha($c->fecha())
        ->setFactor(1)
        ->setMonedaObj('PEN')
        ->setMonedaRef('PEN');

    $detail = new PerceptionDetail();
    $detail->setTipoDoc('01')
        ->setNumDoc('F001-1')
        ->setFechaEmision($c->fecha())
        ->setFechaPercepcion($c->fecha())
        ->setMoneda('PEN')
        ->setImpTotal(200)
        ->setImpCobrar(210)
        ->setImpPercibido(10)
        ->setCobros([$pay])
        ->setTipoCambio($cambio);

    return $perception->setDetails([$detail]);
};
