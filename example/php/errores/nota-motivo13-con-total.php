<?php

declare(strict_types=1);

/* Motivo 13 con importe distinto de cero: SUNAT BETA informó 3315. */

use Greenter\Model\Sale\Note;
use Greenter\Model\Sale\Cuota;
use Greenter\Model\Sale\FormaPagos\FormaPagoCredito;

return static function (Contexto $c): Note {
    /** @var callable(Contexto): Note $construir */
    $construir = require __DIR__.'/../casos/nota-credito.php';

    return $construir($c)
        ->setCorrelativo('901')
        ->setCodMotivo('13')
        ->setFormaPago(new FormaPagoCredito(236))
        ->setCuotas([
            (new Cuota())->setMonto(236)->setFechaPago($c->dias(5))
        ]);
};
