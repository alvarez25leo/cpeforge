<?php

declare(strict_types=1);

/* GRE de transporte público sin fecha de entrega: API GRE informó 3617. */

use Greenter\Model\Despatch\Despatch;

return static function (Contexto $c): Despatch {
    /** @var callable(Contexto): Despatch $construir */
    $construir = require __DIR__.'/../casos/guia-remision.php';
    $guia = $construir($c)->setCorrelativo('901');
    $guia->getEnvio()?->setFecEntregaBienes(null);

    return $guia;
};
