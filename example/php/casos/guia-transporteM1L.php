<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/guia-transporteM1L.php: T001-122, guía 2022 por
 * transporte privado en vehículo de categoría M1 o L.
 */

use Greenter\Model\Client\Client;
use Greenter\Model\Despatch\Despatch;
use Greenter\Model\Despatch\DespatchDetail;
use Greenter\Model\Despatch\Direction;
use Greenter\Model\Despatch\Shipment;

return static function (Contexto $c): Despatch {
    $envio = new Shipment();
    $envio
        ->setCodTraslado('01') // Cat.20 - Venta
        ->setIndicadores(['SUNAT_Envio_IndicadorTrasladoVehiculoM1L'])
        ->setModTraslado('02') // Cat.18 - Transp. Privado
        ->setFecTraslado($c->fecha())
        ->setPesoTotal(12.5)
        ->setUndPesoTotal('KGM')
        ->setLlegada(new Direction('150101', 'AV LIMA'))
        ->setPartida(new Direction('150203', 'AV ITALIA'));

    $despatch = new Despatch();
    $despatch->setVersion('2022')
        ->setTipoDoc('09')
        ->setSerie('T001')
        ->setCorrelativo('122')
        ->setFechaEmision($c->fecha())
        ->setCompany(empresa_gre($c))
        ->setDestinatario((new Client())
            ->setTipoDoc('6')
            ->setNumDoc('20000000002')
            ->setRznSocial('EMPRESA DEST 1'))
        ->setEnvio($envio);

    $detail = new DespatchDetail();
    $detail->setCantidad(0.5)
        ->setUnidad('KGM')
        ->setDescripcion('PROD 1')
        ->setCodigo('PROD1');

    return $despatch->setDetails([$detail]);
};
