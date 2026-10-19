<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/guia-remision.php: T001-121, guía 2022 por
 * transporte público.
 */

use Greenter\Model\Client\Client;
use Greenter\Model\Despatch\Despatch;
use Greenter\Model\Despatch\DespatchDetail;
use Greenter\Model\Despatch\Direction;
use Greenter\Model\Despatch\Shipment;
use Greenter\Model\Despatch\Transportist;

return static function (Contexto $c): Despatch {
    $transp = new Transportist();
    $transp->setTipoDoc('6')
        ->setNumDoc('20000000002')
        ->setRznSocial('TRANSPORTES S.A.C')
        ->setNroMtc('0001');

    $envio = new Shipment();
    $envio
        ->setCodTraslado('01') // Cat.20 - Venta
        ->setModTraslado('01') // Cat.18 - Transp. Publico
        ->setFecTraslado($c->fecha())
        // Campo de greenter/core 5.x ausente en el demo: la API GRE lo exige en
        // transporte público (error 3617).
        ->setFecEntregaBienes($c->fecha())
        ->setPesoTotal(12.5)
        ->setUndPesoTotal('KGM')
    //    ->setNumBultos(2) // Solo válido para importaciones
        ->setLlegada(new Direction('150101', 'AV LIMA'))
        ->setPartida(new Direction('150203', 'AV ITALIA'))
        ->setTransportista($transp);

    $despatch = new Despatch();
    $despatch->setVersion('2022')
        ->setTipoDoc('09')
        ->setSerie('T001')
        ->setCorrelativo('121')
        ->setFechaEmision($c->fecha())
        ->setCompany(empresa_gre($c))
        ->setDestinatario((new Client())
            ->setTipoDoc('6')
            ->setNumDoc('20000000002')
            ->setRznSocial('EMPRESA DEST 1'))
        ->setEnvio($envio);

    $detail = new DespatchDetail();
    $detail->setCantidad(2)
        ->setUnidad('ZZ')
        ->setDescripcion('PROD 1')
        ->setCodigo('PROD1');

    return $despatch->setDetails([$detail]);
};
