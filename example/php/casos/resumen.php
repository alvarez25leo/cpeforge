<?php

declare(strict_types=1);

/*
 * thegreenter/demo examples/resumen.php: RC con dos boletas (una con
 * percepción) y una nota de crédito. Añade `porcentajeIgv`, que SUNAT exige
 * hoy (error 2992) y el demo de 2023 no informa.
 */

use Greenter\Model\Sale\Document;
use Greenter\Model\Summary\Summary;
use Greenter\Model\Summary\SummaryDetail;
use Greenter\Model\Summary\SummaryPerception;

return static function (Contexto $c): Summary {
    $detiail1 = new SummaryDetail();
    $detiail1->setTipoDoc('03')
        ->setSerieNro('B001-1')
        ->setEstado('3')
        ->setClienteTipo('1')
        ->setClienteNro('00000000')
        ->setTotal(129.555)
        ->setMtoOperGravadas(20)
        ->setMtoOperInafectas(24.4)
        ->setMtoOperExoneradas(50)
        ->setMtoOperExportacion(10.555)
        ->setMtoOtrosCargos(21)
        // Campo de greenter/core 5.x ausente en el demo: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        ->setPorcentajeIgv(18)
        ->setMtoIGV(3.6);

    $detiail2 = new SummaryDetail();
    $detiail2->setTipoDoc('07')
        ->setSerieNro('B001-4')
        ->setDocReferencia((new Document())
            ->setTipoDoc('03')
            ->setNroDoc('0001-122'))
        ->setEstado('1')
        ->setClienteTipo('1')
        ->setClienteNro('00000000')
        ->setTotal(200)
        ->setMtoOperGravadas(40)
        ->setMtoOperExoneradas(30)
        ->setMtoOperInafectas(120)
        // Campo de greenter/core 5.x ausente en el demo: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        ->setPorcentajeIgv(18)
        ->setMtoIGV(7.2)
        ->setMtoISC(2.8);

    $detiail3 = new SummaryDetail();
    $detiail3->setTipoDoc('03')
        ->setSerieNro('B001-2')
        ->setEstado('1')
        ->setClienteTipo('1')
        ->setClienteNro('00000000')
        ->setPercepcion((new SummaryPerception())
            ->setCodReg('01')
            ->setTasa(2.00)
            ->setMtoBase(119.00)
            ->setMto(2.38)
            ->setMtoTotal(121.38))
        ->setTotal(119)
        ->setMtoOperGravadas(20)
        ->setMtoOperInafectas(24.4)
        ->setMtoOperExoneradas(50)
        ->setMtoOtrosCargos(21)
        // Campo de greenter/core 5.x ausente en el demo: SUNAT rechaza el RC
        // sin la tasa del IGV de cada línea (error 2992).
        ->setPorcentajeIgv(18)
        ->setMtoIGV(3.6);

    $sum = new Summary();

    // Fecha Generacion menor que Fecha Resumen
    return $sum->setFecGeneracion($c->dias(-3))
        ->setFecResumen($c->dias(-1))
        ->setCorrelativo('001')
        ->setCompany(empresa())
        ->setDetails([$detiail1, $detiail2, $detiail3]);
};
