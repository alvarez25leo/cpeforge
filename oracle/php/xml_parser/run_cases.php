<?php

declare(strict_types=1);

/*
 * Oráculo PHP de greenter/xml-parser v5.3.0: ejecuta los parsers reales del
 * checkout fijado (target/reference/v5.3.0) sobre los XML sintéticos de
 * tests/reference/xml-parser/cases.json y exporta el documento completo con
 * los getters de greenter/core (export_object de oracle/php/core/lib.php).
 *
 * Uso: php oracle/php/xml_parser/run_cases.php <cases.json>
 *
 * Zona horaria del proceso: America/Lima (o la del caso). Las fechas que PHP
 * toma de la hora actual se acotan con `now.before`/`now.after`.
 */

require dirname(__DIR__) . '/core/bootstrap.php';
require dirname(__DIR__) . '/core/lib.php';

$root = dirname(__DIR__, 3);
$referenceDir = getenv('GREENTER_PHP_REFERENCE') ?: $root . '/target/reference/v5.3.0';
$parserSrc = realpath($referenceDir . '/packages/xml-parser/src');
spl_autoload_register(static function (string $class) use ($parserSrc): void {
    if (strncmp($class, 'Greenter\\Xml\\', 13) !== 0) {
        return;
    }
    $file = $parserSrc . '/' . str_replace('\\', '/', substr($class, strlen('Greenter\\'))) . '.php';
    if (is_file($file)) {
        require $file;
    }
});

use Greenter\Xml\Parser\DespatchParser;
use Greenter\Xml\Parser\InvoiceParser;
use Greenter\Xml\Parser\NoteParser;
use Greenter\Xml\Parser\PerceptionParser;
use Greenter\Xml\Parser\ReceiptParser;
use Greenter\Xml\Parser\RetentionParser;
use Greenter\Xml\Parser\SummaryParser;
use Greenter\Xml\Parser\VoidedParser;
use Greenter\Xml\XmlReader;

const PARSERS = [
    'invoice' => InvoiceParser::class, 'note' => NoteParser::class, 'despatch' => DespatchParser::class,
    'receipt' => ReceiptParser::class, 'summary' => SummaryParser::class, 'voided' => VoidedParser::class,
    'retention' => RetentionParser::class, 'perception' => PerceptionParser::class,
];

$warnings = [];
set_error_handler(static function (int $no, string $message) use (&$warnings): bool {
    $warnings[] = preg_replace('/ in \/.* on line \d+$/', '', $message);
    return true;
});

function input_bytes(array $input): string
{
    global $root;
    if (isset($input['xml'])) {
        return $input['xml'];
    }
    if (isset($input['b64'])) {
        return base64_decode($input['b64'], true);
    }
    if (isset($input['file'])) {
        return (string) file_get_contents($root . '/' . $input['file']);
    }
    if (isset($input['repeat'])) {
        $r = $input['repeat'];
        return $r['prefix'] . str_repeat($r['unit'], $r['count']) . $r['suffix'];
    }
    throw new LogicException('entrada desconocida');
}

function micro_now(): array
{
    $d = new DateTime();
    return ['unix' => $d->getTimestamp(), 'micro' => (int) $d->format('u')];
}

function run_parser(array $case): array
{
    $parser = new (PARSERS[$case['parser']])();
    $out = [];
    $inputs = [$case['input']];
    if (isset($case['input2'])) {
        $inputs[] = $case['input2']; // misma instancia, segunda llamada
    }
    foreach ($inputs as $i => $input) {
        $value = input_bytes($input);
        if (($case['mode'] ?? 'string') === 'dom') {
            $doc = new DOMDocument();
            @$doc->loadXML($value);
            $value = $doc;
        }
        $before = micro_now();
        try {
            $document = $parser->parse($value);
            $r = ['class' => $document === null ? null : short_name($document), 'snapshot' => export_value($document)];
        } catch (Throwable $e) {
            $r = ['error' => error_info($e)];
        }
        $r['now'] = ['before' => $before, 'after' => micro_now()];
        $out[] = $r;
    }
    return count($out) === 1 ? $out[0] : ['calls' => $out];
}

function run_reader(array $case): array
{
    $reader = new XmlReader();
    $reader->loadXml(input_bytes($case['input']));
    $results = [];
    foreach ($case['reader'] as $op) {
        try {
            $ctx = null;
            if (isset($op['context'])) {
                $ctx = $reader->getNode($op['context'], null);
            }
            switch ($op['op']) {
                case 'value':
                    $results[] = array_key_exists('default', $op)
                        ? $reader->getValue($op['query'], $ctx, $op['default'])
                        : $reader->getValue($op['query'], $ctx);
                    break;
                case 'node':
                    $node = $reader->getNode($op['query'], $ctx);
                    $results[] = $node === null ? null : ['name' => $node->nodeName, 'value' => $node->nodeValue];
                    break;
                case 'nodes':
                    $nodes = $reader->getNodes($op['query'], $ctx);
                    $list = [];
                    if ($nodes !== false) {
                        foreach ($nodes as $n) {
                            $list[] = ['name' => $n->nodeName, 'value' => $n->nodeValue];
                        }
                    }
                    $results[] = $nodes === false ? ['$false' => true] : $list;
                    break;
            }
        } catch (Throwable $e) {
            $results[] = ['$error' => error_info($e)];
        }
    }
    return ['reader' => $results];
}

// Carga anticipada: las deprecaciones de PHP 8.4 al compilar las clases no
// pertenecen a ningún caso concreto.
foreach (PARSERS as $class) {
    class_exists($class);
}
class_exists(XmlReader::class);
$loadWarnings = array_values(array_unique($warnings));

$cases = json_decode((string) file_get_contents($argv[1]), true, 512, JSON_THROW_ON_ERROR);
$results = [];
foreach ($cases['cases'] as $case) {
    $warnings = [];
    date_default_timezone_set($case['time_zone'] ?? 'America/Lima');
    try {
        $out = isset($case['reader']) ? run_reader($case) : run_parser($case);
    } catch (Throwable $e) {
        $out = ['harness_error' => error_info($e)];
    }
    $out['warnings'] = array_values(array_unique($warnings));
    $results[$case['id']] = $out;
}
$reference = json_decode((string) file_get_contents($root . '/oracle/upstream/referencia.json'), true);
echo json_encode([
    'generator' => 'oracle/php/xml_parser/run_cases.php',
    'reference' => ['tag' => $reference['baseline_tag'], 'commit' => $reference['baseline_commit']],
    'composer_lock_sha256' => hash_file('sha256', dirname(__DIR__) . '/composer.lock'),
    'cases_sha256' => hash_file('sha256', $argv[1]),
    'reference_version' => PHP_VERSION,
    'extensions' => ['dom' => phpversion('dom'), 'libxml' => phpversion('libxml')],
    'libxml' => LIBXML_DOTTED_VERSION,
    'timezone_db' => timezone_version_get(),
    'default_time_zone' => 'America/Lima',
    'load_warnings' => $loadWarnings,
    'results' => $results,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
