<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/resumen-contingencia.php: RC con una boleta de
 * contingencia (serie numérica 0001). Usa el correlativo 002 en lugar del 001
 * del demo y añade `porcentajeIgv`, que SUNAT exige hoy (error 2992).
 */

use Greenter\Model\Summary\Summary;
use Greenter\Model\Summary\SummaryDetail;

return static function (Contexto $c): Summary {
    $sum = new Summary();
    $sum->setFecGeneracion($c->dias(-1))
        ->setFecResumen($c->dias(-1))
        // El demo usa '001', que choca con el caso `resumen` del mismo día.
        ->setCorrelativo('002')
        ->setCompany(empresa());

    $detiail = new SummaryDetail();
    $detiail->setTipoDoc('03')
        ->setSerieNro('0001-1')
        ->setEstado('3')
        ->setClienteTipo('1')
        ->setClienteNro('00000000')
        ->setTotal(119.555)
        ->setMtoOperGravadas(20)
        ->setMtoOperInafectas(24.4)
        ->setMtoOperExoneradas(50)
        ->setMtoOtrosCargos(21.555)
        // Campo de greenter/core 5.x ausente en el demo: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        ->setPorcentajeIgv(18)
        ->setMtoIGV(3.6);

    return $sum->setDetails([$detiail]);
};
