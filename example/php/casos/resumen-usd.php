<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/resumen-usd.php: RC en dólares con una boleta.
 * Añade `porcentajeIgv`, que SUNAT exige hoy (error 2992) y el demo no informa.
 */

use Greenter\Model\Summary\Summary;
use Greenter\Model\Summary\SummaryDetail;

return static function (Contexto $c): Summary {
    $detiail = new SummaryDetail();
    $detiail->setTipoDoc('03')
        ->setSerieNro('B001-1')
        ->setEstado('3')
        ->setClienteTipo('1')
        ->setClienteNro('00000000')
        ->setTotal(129)
        ->setMtoOperGravadas(20)
        ->setMtoOperInafectas(24.4)
        ->setMtoOperExoneradas(50)
        ->setMtoOperExportacion(10)
        ->setMtoOtrosCargos(21)
        // Campo de greenter/core 5.x ausente en el demo: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        ->setPorcentajeIgv(18)
        ->setMtoIGV(3.6);

    $sum = new Summary();

    return $sum->setFecGeneracion($c->dias(-3))
        ->setFecResumen($c->dias(-1))
        ->setCorrelativo('211')
        ->setMoneda('USD')
        ->setCompany(empresa())
        ->setDetails([$detiail]);
};
