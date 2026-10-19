<?php

declare(strict_types=1);

/*
 * Oráculo de los filtros y funciones de greenter/xml v5.3.0:
 * FormatFilter::number (n_format), FormatFilter::numberLimit (n_format_limit),
 * TributoFunction::getByAfectacion y getByTributo, y la impresión `{{ x }}`
 * de un float en Twig (conversión float→string con precision=14).
 *
 * Uso: php oracle/php/xml/filters.php tests/reference/xml/filters/inputs.json
 */

require dirname(__DIR__) . '/core/lib.php';

$vendor = getenv('GREENTER_PHP_ORACLE_VENDOR') ?: dirname(__DIR__, 3) . '/target/php-oracle-vendor';
require $vendor . '/autoload.php';
$xmlSrc = realpath(GREENTER_CORE_SRC . '/../../../xml/src');
require $xmlSrc . '/Xml/Filter/FormatFilter.php';
require $xmlSrc . '/Xml/Filter/TributoFunction.php';

use Greenter\Xml\Filter\FormatFilter;
use Greenter\Xml\Filter\TributoFunction;
use Twig\Environment;
use Twig\Loader\ArrayLoader;

ini_set('precision', '14');
ini_set('serialize_precision', '-1');

$deprecations = [];
set_error_handler(static function (int $errno, string $message) use (&$deprecations): bool {
    if ($errno === E_DEPRECATED || $errno === E_USER_DEPRECATED) {
        $deprecations[] = $message;
        return true;
    }
    return false;
});

$twig = new Environment(new ArrayLoader(['print' => '{{ value }}']), ['autoescape' => 'html']);
$format = new FormatFilter();
$inputs = json_decode((string) file_get_contents($argv[1] ?? ''), true, 512, JSON_THROW_ON_ERROR);
$results = [];
foreach ($inputs['inputs'] as $input) {
    $deprecations = [];
    $value = hydrate($input['value'] ?? null);
    try {
        $output = match ($input['fn']) {
            'n_format' => array_key_exists('decimals', $input) ? $format->number($value, $input['decimals']) : $format->number($value),
            'n_format_limit' => $format->numberLimit($value, $input['decimals']),
            'print' => $twig->render('print', ['value' => $value]),
            'tributo_afectacion' => TributoFunction::getByAfectacion($value),
            'tributo' => TributoFunction::getByTributo($value),
        };
        $results[] = ['input' => $input, 'output' => $output, 'deprecations' => $deprecations];
    } catch (Throwable $e) {
        $results[] = ['input' => $input, 'error' => ['class' => get_class($e), 'message' => $e->getMessage()]];
    }
}

echo json_encode([
    'generator' => 'oracle/php/xml/filters.php',
    'reference_version' => PHP_VERSION,
    'results' => $results,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_PRESERVE_ZERO_FRACTION | JSON_THROW_ON_ERROR), "\n";
