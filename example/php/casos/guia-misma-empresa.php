<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/guia-misma-empresa.php: T001-120, guía 2022 de
 * traslado entre establecimientos de la misma empresa en vehículo M1/L.
 */

use Greenter\Model\Client\Client;
use Greenter\Model\Despatch\Despatch;
use Greenter\Model\Despatch\DespatchDetail;
use Greenter\Model\Despatch\Direction;
use Greenter\Model\Despatch\Shipment;

// El demo usa 20123456789, su emisor GRE de entonces. En un traslado entre
// establecimientos de la misma empresa, el destinatario y los anexos de
// partida y llegada deben ser el emisor (errores 2554 y 3414).
return static function (Contexto $c): Despatch {
    $envio = new Shipment();
    $envio
        ->setCodTraslado('04') // Cat.20 - Traslado entre establecimientos de la misma empresa
        ->setModTraslado('02') // Cat.18 - Transp. Privado
        ->setIndicadores(['SUNAT_Envio_IndicadorTrasladoVehiculoM1L']) // Transp M1 y L
        ->setFecTraslado($c->fecha())
        ->setPesoTotal(12.5)
        ->setUndPesoTotal('KGM')
        ->setLlegada((new Direction('150101', 'AV LIMA'))
            ->setRuc($c->greRuc)
            ->setCodLocal('00002')) // Código de establecimiento anexo
        ->setPartida((new Direction('150203', 'AV ITALIA'))
            ->setRuc($c->greRuc)
            ->setCodLocal('00001'));

    $despatch = new Despatch();
    $despatch->setVersion('2022')
        ->setTipoDoc('09')
        ->setSerie('T001')
        ->setCorrelativo('120')
        ->setFechaEmision($c->fecha())
        ->setCompany(empresa_gre($c))
        ->setDestinatario((new Client())
            ->setTipoDoc('6')
            ->setNumDoc($c->greRuc)
            ->setRznSocial('GREENTER SAC')) // misma empresa
        ->setEnvio($envio);

    $detail = new DespatchDetail();
    $detail->setCantidad(2)
        ->setUnidad('ZZ')
        ->setDescripcion('PROD 1')
        ->setCodigo('PROD1');

    return $despatch->setDetails([$detail]);
};
