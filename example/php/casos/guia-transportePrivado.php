<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/guia-transportePrivado.php: T001-123, guía 2022
 * por transporte privado con vehículo principal, secundario y conductor.
 */

use Greenter\Model\Client\Client;
use Greenter\Model\Despatch\Despatch;
use Greenter\Model\Despatch\DespatchDetail;
use Greenter\Model\Despatch\Direction;
use Greenter\Model\Despatch\Driver;
use Greenter\Model\Despatch\Shipment;
use Greenter\Model\Despatch\Vehicle;

return static function (Contexto $c): Despatch {
    $vehiculoSecundario = (new Vehicle())
        ->setPlaca('ABI456');

    $vehiculoPrincipal = (new Vehicle())
        ->setPlaca('ABC123')
        ->setSecundarios([$vehiculoSecundario]); // opcional

    $chofer = (new Driver())
        ->setTipo('Principal')
        ->setTipoDoc('1')
        ->setNroDoc('44004400')
        ->setLicencia('0001122020')
        ->setNombres('ROBERTO')
        ->setApellidos('RODRIGUEZ VALENCIA');

    $envio = new Shipment();
    $envio
        ->setCodTraslado('01') // Cat.20 - Venta
        ->setModTraslado('02') // Cat.18 - Transp. Privado
        ->setFecTraslado($c->fecha())
        ->setPesoTotal(12.5)
        ->setUndPesoTotal('KGM')
        ->setVehiculo($vehiculoPrincipal)
        ->setChoferes([$chofer])
        ->setLlegada(new Direction('150101', 'AV LIMA'))
        ->setPartida(new Direction('150203', 'AV ITALIA'));

    $despatch = new Despatch();
    $despatch->setVersion('2022')
        ->setTipoDoc('09')
        ->setSerie('T001')
        ->setCorrelativo('123')
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
