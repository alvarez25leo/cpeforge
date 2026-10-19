<?php

declare(strict_types=1);

// Ejecuta el proveedor real del checkout fijado. Ningún código del catálogo se
// reimplementa aquí. DOM solo conserva la forma textual de las claves que PHP
// convierte en enteros al construir getAll().
$root = dirname(__DIR__, 3);
$reference = getenv('PHP_REFERENCE_DIR') ?: $root.'/target/reference/v5.3.0';
require $reference.'/packages/core/src/Core/Validator/ErrorCodeProviderInterface.php';
require $reference.'/packages/xcodes/src/Validator/XmlErrorCodeProvider.php';

$provider = new Greenter\Validator\XmlErrorCodeProvider();
$all = $provider->getAll();
$dom = new DOMDocument();
if (!$dom->load($reference.'/packages/xcodes/src/data/CodeErrors.xml')) {
    throw new RuntimeException('No se pudo cargar el XML fijado');
}
$nodes = (new DOMXPath($dom))->query('/errors/error');
if ($all === null || $nodes === false || count($all) !== $nodes->length) {
    throw new RuntimeException('getAll o XML incompleto');
}

$allKeys = array_keys($all);
$allValues = array_values($all);
$rows = [];
$queries = [];
foreach ($nodes as $index => $node) {
    $code = $node->getAttribute('code');
    $rows[] = [
        'xml_code' => $code,
        'reference_key' => $allKeys[$index],
        'reference_key_type' => gettype($allKeys[$index]),
        'message' => $allValues[$index],
    ];
    $queries[] = ['id' => 'all/'.$code, 'code' => $code, 'value' => $provider->getValue($code)];
}

$extra = [
    'null' => null,
    'empty' => '',
    'space' => ' ',
    'leading-zero' => '0100',
    'extra-zero' => '000100',
    'canonical' => '1001',
    'unknown' => '9999',
    'alpha' => 'A110',
    'unicode' => 'á中',
    'newline' => "0100\n",
    'long' => str_repeat('9', 4096),
    'xpath-no-match' => "' and @code='0100",
    'xpath-many' => "' or '1'='1",
    'xpath-injection-0100' => "' or @code='0100",
];
foreach ($extra as $id => $code) {
    $queries[] = ['id' => $id, 'code' => $code, 'value' => $provider->getValue($code)];
}

echo json_encode(
    ['source' => 'thegreenter/greenter v5.3.0 f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f',
     'all' => $rows, 'values' => $queries],
    JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR
), "\n";
