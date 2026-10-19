<?php
/** Diferencias de seguridad: PHP upstream real, entradas locales sintéticas. */
declare(strict_types=1);
$workspace = dirname(__DIR__, 3);
$source = $workspace . '/target/reference/ubl-validator-v2.3.0/src';
$corpus = $workspace . '/tests/reference/ubl-validator/xml';
// Prefijo del workspace → `{workspace}/` solo en `error` y `errors[].message`,
// como en run_cases.php.
$relative = static fn (?string $text): ?string =>
    $text === null ? null : str_replace($workspace . '/', '{workspace}/', $text);
spl_autoload_register(static function (string $class) use ($source): void {
    if (strncmp($class, 'Greenter\\', 9) !== 0) return;
    $path = $source . '/' . str_replace('\\', '/', substr($class, 9)) . '.php';
    if (is_file($path)) require_once $path;
});
$out = [];
foreach ([
    'ubl/dtd-internal' => 'security-dtd-internal.xml',
    'ubl/version-traversal' => 'security-version-traversal.xml',
    'ubl/deep-nesting' => 'security-deep-nesting.xml',
] as $id => $file) {
    libxml_clear_errors();
    $validator = new Greenter\Ubl\UblValidator();
    $valid = $validator->isValid(file_get_contents($corpus . '/' . $file));
    $out[$id] = ['file' => $file, 'valid' => $valid, 'error' => $relative($validator->getError())];
}
$doc = new DOMDocument();
$doc->loadXML('<Signature xmlns="http://www.w3.org/2000/09/xmldsig#"/>');
$validator = new Greenter\Ubl\SchemaValidator();
libxml_clear_errors();
$valid = $validator->validate($doc, $source . '/xsd/2.0/common/xmldsig-core-schema.xsd');
$errors = [];
foreach ($validator->getErrors() as $error) {
    $errors[] = ['level' => $error->level, 'code' => $error->code,
        'column' => $error->column, 'message' => $relative($error->message),
        'file' => $error->file, 'line' => $error->line];
}
$out['ubl/xades-missing-import'] = ['valid' => $valid, 'errors' => $errors];
$doc = new DOMDocument();
$doc->loadXML('<probe>synthetic</probe>');
$validator = new Greenter\Ubl\SchemaValidator();
$valid = $validator->validate($doc, $workspace . '/tests/reference/ubl-validator/probe.xsd');
$out['ubl/schema-outside-manifest'] = ['valid' => $valid, 'error_count' => count($validator->getErrors())];
echo json_encode(['reference' => '980bad9412eab51516e4879b06d8d2e723c62f0d',
    'runtime' => PHP_VERSION, 'libxml' => LIBXML_DOTTED_VERSION,
    'results' => $out], JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
