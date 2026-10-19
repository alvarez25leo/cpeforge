<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/retencion.php: R001-123.
 */

use Greenter\Model\Retention\Exchange;
use Greenter\Model\Retention\Payment;
use Greenter\Model\Retention\Retention;
use Greenter\Model\Retention\RetentionDetail;

return static function (Contexto $c): Retention {
    $retention = new Retention();
    $retention
        ->setSerie('R001')
        ->setCorrelativo('123')
        ->setFechaEmision($c->fecha())
        ->setCompany(empresa())
        ->setProveedor(cliente())
        ->setObservacion('NOTA /><!-- HI -->')
        ->setImpRetenido(10)
        ->setImpPagado(200)
        ->setRegimen('01')
        ->setTasa(3);

    $pay = new Payment();
    $pay->setMoneda('PEN')
        ->setFecha($c->fecha())
        ->setImporte(100);

    $cambio = new Exchange();
    $cambio->setFecha($c->fecha())
        ->setFactor(1)
        ->setMonedaObj('PEN')
        ->setMonedaRef('PEN');

    $detail = new RetentionDetail();
    $detail->setTipoDoc('01')
        ->setNumDoc('F001-1')
        ->setFechaEmision($c->fecha())
        ->setFechaRetencion($c->fecha())
        ->setMoneda('PEN')
        ->setImpTotal(210)
        ->setImpPagar(200)
        ->setImpRetenido(10)
        ->setPagos([$pay])
        ->setTipoCambio($cambio);

    return $retention->setDetails([$detail]);
};
