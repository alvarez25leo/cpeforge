<?php
/** Oráculo: clases originales greenter/ubl-validator v2.3.0, sin XML upstream. */
declare(strict_types=1);

$workspace = dirname(__DIR__, 3);
$upstream = $workspace . '/target/reference/ubl-validator-v2.3.0';
$source = $upstream . '/src';
$data = $workspace . '/tests/reference/ubl-validator';
// Upstream incrusta en getError() y libxml en sus mensajes la ruta absoluta del
// XSD. Solo en los campos `error` y `errors[].message` se sustituye el prefijo
// exacto del workspace por `{workspace}/`, para que el fixture no dependa del
// checkout; crates/ubl-validator/tests/parity.rs lo expande en los mismos campos.
$relative = static fn (?string $text): ?string =>
    $text === null ? null : str_replace($workspace . '/', '{workspace}/', $text);
spl_autoload_register(static function (string $class) use ($source): void {
    if (strncmp($class, 'Greenter\\', 9) !== 0) return;
    $path = $source . '/' . str_replace('\\', '/', substr($class, 9)) . '.php';
    if (is_file($path)) require_once $path;
});

$cases = json_decode(file_get_contents($data . '/cases.json'), true, 512, JSON_THROW_ON_ERROR)['cases'];
$results = [];
foreach ($cases as $case) {
    libxml_clear_errors();
    $xml = file_get_contents($data . '/xml/' . $case['file']);
    $doc = new DOMDocument();
    if ($xml !== '') @$doc->loadXML($xml);
    $resolver = new Greenter\Ubl\Resolver\UblVersionResolver();
    $version = $resolver->getVersion($doc);
    $relativePath = $doc->documentElement === null ? null :
        $version . '/maindoc/UBL-' . $doc->documentElement->localName . '-' . $version . '.xsd';
    $validator = new Greenter\Ubl\UblValidator();
    $before = $validator->getError();
    try {
        $valid = $validator->isValid(($case['mode'] ?? 'string') === 'dom' ? $doc : $xml);
        $errors = [];
        // UblValidator::getErrorMessage ya consumió el Generator del primer
        // SchemaValidator. Ejecutar el SchemaValidator original otra vez con
        // el mismo DOM/XSD para obtener sus campos ordenados sin alterar PHP.
        if (!$valid && $doc->documentElement !== null) {
            $schemaPath = $source . '/xsd/' . $version . '/maindoc/UBL-' . $doc->documentElement->localName . '-' . $version . '.xsd';
            if (is_file($schemaPath)) {
                libxml_clear_errors();
                $standalone = new Greenter\Ubl\SchemaValidator();
                $standalone->validate($doc, $schemaPath);
                foreach ($standalone->getErrors() as $error) {
                $errors[] = [
                    'level' => $error->level, 'code' => $error->code,
                    'column' => $error->column, 'message' => $relative($error->message),
                    'file' => $error->file, 'line' => $error->line,
                ];
                }
            }
        }
        $results[] = [
            'id' => $case['id'], 'mode' => $case['mode'] ?? 'string',
            'version' => $version, 'schema' => $relativePath, 'valid' => $valid,
            'before' => $before, 'error' => $relative($validator->getError()), 'errors' => $errors,
        ];
    } catch (Throwable $error) {
        $results[] = [
            'id' => $case['id'], 'mode' => $case['mode'] ?? 'string',
            'version' => $version, 'schema' => $relativePath, 'exception' => get_class($error),
            'message' => $error->getMessage(),
        ];
    }
}
$stateValidator = new Greenter\Ubl\UblValidator();
$state = ['before' => $stateValidator->getError(), 'steps' => []];
foreach (['2.0-Invoice.xml', '2.1-Invoice.xml', '2.1-Invoice-missing-version.xml'] as $file) {
    $valid = $stateValidator->isValid(file_get_contents($data . '/xml/' . $file));
    $state['steps'][] = [
        'file' => $file, 'valid' => $valid,
        'error' => $relative($stateValidator->getError()),
        'cached_version' => $stateValidator->pathResolver->version,
    ];
}
$stateValidator->pathResolver->version = '2.1';
$valid = $stateValidator->isValid(file_get_contents($data . '/xml/2.1-Invoice.xml'));
$state['steps'][] = [
    'file' => '2.1-Invoice.xml', 'valid' => $valid,
    'error' => $relative($stateValidator->getError()),
    'cached_version' => $stateValidator->pathResolver->version,
];
echo json_encode([
    'reference' => '980bad9412eab51516e4879b06d8d2e723c62f0d',
    'runtime' => PHP_VERSION, 'libxml' => LIBXML_DOTTED_VERSION,
    'results' => $results, 'state' => $state,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
