<?php

declare(strict_types=1);

/* RC deliberadamente sin porcentaje de IGV por línea: SUNAT BETA informó 2992. */

use Greenter\Model\Summary\Summary;

return static function (Contexto $c): Summary {
    /** @var callable(Contexto): Summary $construir */
    $construir = require __DIR__.'/../casos/resumen.php';
    $resumen = $construir($c)->setCorrelativo('901');
    foreach ($resumen->getDetails() ?? [] as $detalle) {
        $detalle->setPorcentajeIgv(null);
    }

    return $resumen;
};
