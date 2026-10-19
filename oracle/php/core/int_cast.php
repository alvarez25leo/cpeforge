<?php

declare(strict_types=1);

/*
 * Oráculo de la conversión `(int) $string` de PHP que usa
 * CdrResponse::isAccepted(). Lee una lista de cadenas y emite el entero.
 *
 * Uso: php oracle/php/core/int_cast.php tests/reference/core/int-cast/inputs.json
 */

$inputs = json_decode((string) file_get_contents($argv[1] ?? ''), true, 512, JSON_THROW_ON_ERROR);
$results = [];
foreach ($inputs['inputs'] as $text) {
    $results[] = ['input' => $text, 'int' => (int) $text];
}
echo json_encode([
    'generator' => 'oracle/php/core/int_cast.php',
    'reference_version' => PHP_VERSION,
    'reference_int_size' => PHP_INT_SIZE,
    'results' => $results,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
