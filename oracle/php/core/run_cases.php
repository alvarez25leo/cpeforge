<?php

declare(strict_types=1);

/*
 * Oráculo PHP de greenter/core.
 *
 * Lee un archivo de casos sintéticos (tests/reference/core/cases/*.json),
 * construye los objetos llamando a los constructores y setters reales de la
 * referencia fijada y emite, por caso, una instantánea JSON de todo lo
 * observable: getters públicos, métodos derivados y errores.
 *
 * Uso: php oracle/php/core/run_cases.php <casos.json>
 *
 * Gramática de entrada (nodos):
 *   {"$class": "Invoice", "$ctor": [..]?, "<propiedad>": <nodo>, ...}
 *       -> new Clase(...$ctor) y luego set<Propiedad>(<nodo>) en orden.
 *   {"$decimal": "18.50"}  -> float PHP mediante conversión de cadena numérica.
 *   {"$bytes": "UEsDBA=="} -> cadena PHP binaria (Base64 estándar).
 *   {"$datetime": "2024-01-31T23:30:00-05:00", "tz": "America/Lima",
 *    "class": "DateTimeImmutable"|"DateTime"}
 *       -> instante RFC 3339 trasladado a la zona `tz` con setTimezone().
 *   listas JSON -> arrays PHP; escalares JSON -> escalares PHP.
 *
 * El archivo declara strict_types=1: un tipo incorrecto en un fixture produce
 * TypeError en lugar de una coerción silenciosa.
 */

require __DIR__ . '/lib.php';

ini_set('serialize_precision', '-1');
ini_set('precision', '17');

function run_case(array $case): array
{
    try {
        $root = hydrate($case['input']);
    } catch (Throwable $e) {
        return ['hydrate_error' => error_info($e)];
    }
    $calls = [];
    foreach ($case['calls'] ?? [] as $call) {
        try {
            $calls[$call] = export_value($root->$call());
        } catch (Throwable $e) {
            $calls[$call] = ['$error' => error_info($e)];
        }
    }
    // La instantánea se toma después de las llamadas: así se detecta cualquier
    // mutación de estado provocada por métodos derivados (p. ej. zona horaria).
    return ['calls' => (object) $calls, 'snapshot' => export_value($root)];
}

$file = $argv[1] ?? fail('uso: run_cases.php <casos.json>');
$cases = json_decode((string) file_get_contents($file), true, 512, JSON_THROW_ON_ERROR);
date_default_timezone_set('UTC');

$results = [];
foreach ($cases['cases'] as $case) {
    if (isset($results[$case['id']])) {
        fail("id duplicado: {$case['id']}");
    }
    $results[$case['id']] = run_case($case);
}

$reference = json_decode((string) file_get_contents(dirname(__DIR__, 3) . '/oracle/upstream/referencia.json'), true);
echo json_encode([
    'generator' => 'oracle/php/core/run_cases.php',
    'reference' => ['tag' => $reference['baseline_tag'], 'commit' => $reference['baseline_commit']],
    'reference_version' => PHP_VERSION,
    'timezone_db' => timezone_version_get(),
    'cases_file' => basename($file),
    'results' => $results,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_PRESERVE_ZERO_FRACTION | JSON_THROW_ON_ERROR), "\n";
