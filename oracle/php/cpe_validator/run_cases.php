<?php

declare(strict_types=1);

/*
 * Oráculo PHP de greenter/cpe-validator v5.3.0: ejecuta las clases reales del
 * checkout fijado (target/reference/v5.3.0/packages/cpe-validator) sobre
 * los casos sintéticos de tests/reference/cpe-validator/cases.json.
 *
 * Uso: php oracle/php/cpe_validator/run_cases.php <cases.json>
 *
 * Cada caso materializa sus hojas en un directorio temporal propio y ejecuta
 * sus pasos en orden sobre las mismas instancias. Antes de cada caso se
 * limpia el estado global de libxml (errores pendientes y
 * libxml_use_internal_errors), porque XslValidator no lo restaura cuando una
 * excepción interrumpe la transformación: dentro de un caso esa fuga es
 * observable y se conserva a propósito (xsl-reuse-after-exception).
 * Las rutas temporales se sustituyen por {ROOT} y {OUTSIDE}.
 */

require dirname(__DIR__) . '/core/lib.php';

$root = dirname(__DIR__, 3);
$referenceDir = getenv('GREENTER_PHP_REFERENCE') ?: $root . '/target/reference/v5.3.0';
$validatorSrc = realpath($referenceDir . '/packages/cpe-validator/src');
if ($validatorSrc === false) {
    fail("No existe packages/cpe-validator en $referenceDir");
}
spl_autoload_register(static function (string $class) use ($validatorSrc): void {
    if (strncmp($class, 'Greenter\\Validator\\', 18) !== 0) {
        return;
    }
    $file = $validatorSrc . '/' . str_replace('\\', '/', substr($class, strlen('Greenter\\'))) . '.php';
    if (is_file($file)) {
        require $file;
    }
});

use Greenter\Validator\Entity\CpeError;
use Greenter\Validator\Parser\ErrorResultParser;
use Greenter\Validator\Resolver\TypeResolverInterface;
use Greenter\Validator\Resolver\XmlTypeResolver;
use Greenter\Validator\Resolver\XslPathResolver;
use Greenter\Validator\Xml\CpeValidator;
use Greenter\Validator\Xml\XslValidator;

$warnings = [];
set_error_handler(static function (int $no, string $message) use (&$warnings): bool {
    $warnings[] = preg_replace('/ in \/.* on line \d+$/', '', $message);
    return true;
});

/** Sustituciones de rutas temporales del caso en curso. */
$replacements = [];

function normalize(mixed $value): mixed
{
    global $replacements;
    if (is_string($value)) {
        return strtr($value, $replacements);
    }
    if (is_array($value)) {
        return array_map('normalize', $value);
    }
    return $value;
}

function input_xml(?array $input): ?string
{
    global $root;
    if ($input === null) {
        return null;
    }
    if (array_key_exists('xml', $input)) {
        return $input['xml'];
    }
    if (isset($input['file'])) {
        return (string) file_get_contents($root . '/' . $input['file']);
    }
    throw new LogicException('entrada desconocida');
}

function export_errors(array $errors): array
{
    return ['outcome' => 'errors', 'errors' => array_map(static fn (CpeError $e): array => [
        $e->getCode(), $e->getMessage(), $e->getLevel(), $e->getNodePath(), $e->getNodeValue(),
    ], $errors)];
}

function exception_outcome(Throwable $e): array
{
    return ['outcome' => 'exception'] + error_info($e);
}

function remove_tree(string $dir): void
{
    if (!is_dir($dir) || is_link($dir)) {
        @unlink($dir);
        return;
    }
    foreach (scandir($dir) as $entry) {
        if ($entry !== '.' && $entry !== '..') {
            remove_tree($dir . '/' . $entry);
        }
    }
    rmdir($dir);
}

function materialize(string $dir, array $files, array $vars): void
{
    foreach ($files as $rel => $content) {
        $path = $dir . '/' . $rel;
        if (!is_dir(dirname($path))) {
            mkdir(dirname($path), 0777, true);
        }
        file_put_contents($path, strtr($content, $vars));
    }
}

/** Resolutor de tipo de prueba: devuelve un tipo sin ruta (como el mock de CpeValidatorTest). */
final class UnmappedTypeResolver implements TypeResolverInterface
{
    public function getType(DOMDocument $doc): ?string
    {
        return '00';
    }

    public function getTypeFromXml(?string $xml): ?string
    {
        return '00';
    }
}

function run_case(array $case, string $work): array
{
    global $replacements;
    $rules = $work . '/rules';
    $outside = $work . '/outside';
    mkdir($rules, 0777, true);
    mkdir($outside, 0777, true);
    $replacements = [];
    foreach ([[$rules, '{ROOT}'], [$outside, '{OUTSIDE}']] as [$dir, $token]) {
        $real = realpath($dir);
        $replacements[$real] = $token;
        $replacements[$dir] = $token;
    }
    $vars = ['{ROOT}' => $rules, '{OUTSIDE}' => $outside];
    materialize($rules, $case['rules'] ?? [], $vars);
    materialize($outside, $case['outside'] ?? [], $vars);
    foreach ($case['symlinks'] ?? [] as $rel => $target) {
        $link = $rules . '/' . $rel;
        if (!is_dir(dirname($link))) {
            mkdir(dirname($link), 0777, true);
        }
        symlink(strtr($target, $vars), $link);
    }

    libxml_clear_errors();
    libxml_use_internal_errors(false);

    $parser = new ErrorResultParser();
    $xslValidator = new XslValidator($parser);
    $typeResolver = ($case['resolver'] ?? 'default') === 'unmapped_type' ? new UnmappedTypeResolver() : new XmlTypeResolver();
    $cpe = new CpeValidator($typeResolver, new XslPathResolver($rules), new XslValidator($parser));

    $steps = [];
    foreach ($case['steps'] as $step) {
        try {
            switch ($step['op']) {
                case 'parse':
                    $error = $parser->parse($step['raw']);
                    $steps[] = $error === null ? ['outcome' => 'null'] : export_errors([$error]);
                    break;
                case 'path':
                    $steps[] = ['outcome' => 'value', 'value' => (new XslPathResolver($step['base']))->getPath($step['code'])];
                    break;
                case 'type_xml':
                    $steps[] = ['outcome' => 'value', 'value' => (new XmlTypeResolver())->getTypeFromXml(input_xml($step['xml']))];
                    break;
                case 'type':
                case 'rule':
                    $type = (new XmlTypeResolver())->getTypeFromXml(input_xml($step['xml']));
                    $value = $step['op'] === 'type' ? $type : (new XslPathResolver($rules))->getPath($type);
                    $steps[] = ['outcome' => 'value', 'value' => $value];
                    break;
                case 'xsl_set':
                    $xsl = new DOMDocument();
                    $xsl->load($rules . '/' . $step['path']);
                    $xslValidator->setXsl($xsl);
                    $steps[] = ['outcome' => 'ok'];
                    break;
                case 'xsl_validate':
                    $doc = new DOMDocument();
                    $doc->loadXML(input_xml($step['xml']));
                    $steps[] = export_errors($xslValidator->validate($step['filename'], $doc));
                    break;
                case 'cpe_validate':
                    $xml = input_xml($step['xml']);
                    if (($step['mode'] ?? 'xml') === 'dom') {
                        $doc = new DOMDocument();
                        $doc->loadXML($xml);
                        $steps[] = export_errors($cpe->validate($step['filename'], $doc));
                    } else {
                        $steps[] = export_errors($cpe->validateFromXml($step['filename'], $xml));
                    }
                    break;
                case 'exists':
                    $steps[] = ['outcome' => 'value', 'value' => file_exists($rules . '/' . $step['path'])];
                    break;
                default:
                    throw new LogicException('paso desconocido: ' . $step['op']);
            }
        } catch (Throwable $e) {
            $steps[] = exception_outcome($e);
        }
    }
    return normalize(['steps' => $steps]);
}

// Carga anticipada de las 14 clases para que ningún aviso de compilación se
// atribuya a un caso.
$it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($validatorSrc, FilesystemIterator::SKIP_DOTS));
$classes = [];
foreach ($it as $file) {
    $rel = substr($file->getPathname(), strlen($validatorSrc) + 1, -4);
    $classes[] = 'Greenter\\' . str_replace('/', '\\', $rel);
}
sort($classes);
foreach ($classes as $class) {
    class_exists($class) || interface_exists($class) || fail("no se pudo cargar $class");
}
$loadWarnings = array_values(array_unique($warnings));

$cases = json_decode((string) file_get_contents($argv[1]), true, 512, JSON_THROW_ON_ERROR);
$tmp = sys_get_temp_dir() . '/greenter-cpe-oracle-' . getmypid();
remove_tree($tmp);
$results = [];
foreach ($cases['cases'] as $case) {
    $warnings = [];
    $work = $tmp . '/' . $case['id'];
    try {
        $out = run_case($case, $work);
    } catch (Throwable $e) {
        $out = ['harness_error' => error_info($e)];
    }
    $out['warnings'] = normalize(array_values(array_unique($warnings)));
    $results[$case['id']] = $out;
    remove_tree($work);
}
remove_tree($tmp);
libxml_clear_errors();

$reference = json_decode((string) file_get_contents($root . '/oracle/upstream/referencia.json'), true);
echo json_encode([
    'generator' => 'oracle/php/cpe_validator/run_cases.php',
    'reference' => ['tag' => $reference['baseline_tag'], 'commit' => $reference['baseline_commit']],
    'cases_sha256' => hash_file('sha256', $argv[1]),
    'classes' => $classes,
    'reference_version' => PHP_VERSION,
    'extensions' => ['dom' => phpversion('dom'), 'libxml' => phpversion('libxml'), 'xsl' => phpversion('xsl'), 'pcre' => phpversion('pcre')],
    'libxml' => LIBXML_DOTTED_VERSION,
    'libxslt' => LIBXSLT_DOTTED_VERSION,
    'libexslt' => LIBEXSLT_DOTTED_VERSION,
    'pcre' => PCRE_VERSION,
    'ini' => ['pcre.jit' => ini_get('pcre.jit'), 'pcre.backtrack_limit' => ini_get('pcre.backtrack_limit'),
        'pcre.recursion_limit' => ini_get('pcre.recursion_limit'), 'precision' => ini_get('precision')],
    'load_warnings' => $loadWarnings,
    'results' => $results,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
