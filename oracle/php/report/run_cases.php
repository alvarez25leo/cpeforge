<?php

declare(strict_types=1);

/*
 * Oráculo PHP de greenter/report v5.3.0.
 *
 * Ejecuta los casos neutrales de oracle/cases/report con el código PHP real:
 * HtmlReport (Twig 3.30.0), QrRender/Bacon QR 3.1.1, XmlUtils,
 * DefaultTemplateResolver y DocumentFilter. Guarda las salidas sin modificar.
 *
 * Uso:
 *   php -n run_cases.php html <casos.json> <dir_casos> <salida>   (un subproceso por caso)
 *   php -n run_cases.php qr <casos.json> <salida>
 *   php -n run_cases.php xml <casos.json> <salida>
 *   php -n run_cases.php resolver <casos.json>
 *   php -n run_cases.php catalog
 */

require dirname(__DIR__) . '/core/lib.php';

$vendor = getenv('GREENTER_REPORT_ORACLE_VENDOR') ?: dirname(__DIR__, 3) . '/target/php-report-vendor';
if (!is_file($vendor . '/autoload.php')) {
    fail("Falta $vendor/autoload.php; ejecutar composer install en oracle/php/report");
}
require $vendor . '/autoload.php';

$reportSrc = realpath(GREENTER_CORE_SRC . '/../../../report/src');
if ($reportSrc === false) {
    fail('No se encontró packages/report/src en la referencia');
}
define('GREENTER_REPORT_SRC', $reportSrc);
spl_autoload_register(static function (string $class) use ($reportSrc): void {
    if (strncmp($class, 'Greenter\\Report\\', 16) !== 0) {
        return;
    }
    $file = $reportSrc . '/' . str_replace('\\', '/', substr($class, strlen('Greenter\\'))) . '.php';
    if (is_file($file)) {
        require $file;
    }
});

// Valores por defecto de PHP, fijados para no depender del php.ini local.
ini_set('precision', '14');
ini_set('serialize_precision', '-1');
date_default_timezone_set('UTC');

use Greenter\Report\HtmlReport;
use Greenter\Report\Resolver\DefaultTemplateResolver;
use Greenter\Report\XmlUtils;
use Greenter\Report\Filter\DocumentFilter;
use BaconQrCode\Common\ErrorCorrectionLevel;
use BaconQrCode\Encoder\Encoder;
use BaconQrCode\Renderer\Image\SvgImageBackEnd;
use BaconQrCode\Renderer\ImageRenderer;
use BaconQrCode\Renderer\RendererStyle\RendererStyle;
use BaconQrCode\Writer;

const JSON_FLAGS = JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR;

$casesRoot = null;

/** Quita rutas locales de los mensajes para que el fixture sea estable. */
function clean(string $message): string
{
    global $casesRoot;
    $message = preg_replace('/, called in \S+ on line \d+/', '', $message);
    // Código evaluado por Twig: "called in <vendor>/Environment.php(434) : eval()'d code on line N".
    $message = preg_replace("/, called in \\S+\\(\\d+\\) : eval\\(\\)'d code on line \\d+/", '', $message);
    if ($casesRoot !== null) {
        $message = str_replace($casesRoot, '<casos>', $message);
    }
    $message = str_replace(GREENTER_REPORT_SRC, '<report-src>', $message);
    return str_replace(dirname(GREENTER_CORE_SRC, 4), '<ref>', $message);
}

function throwable_info(Throwable $e): array
{
    $info = ['class' => get_class($e), 'message' => clean($e->getMessage())];
    if ($e->getPrevious() !== null) {
        $info['previous'] = throwable_info($e->getPrevious());
    }
    return $info;
}

/** Parámetros: arrays PHP con los valores de la gramática neutral. */
function param_node(mixed $node): mixed
{
    if (!is_array($node)) {
        return $node;
    }
    if (array_is_list($node)) {
        return array_map('param_node', $node);
    }
    if (array_key_exists('$repeat', $node)) {
        // Cadena larga sin guardarla en el fixture: el patrón se repite hasta `length` bytes.
        $chunk = hydrate($node['$repeat']);
        return substr(str_repeat($chunk, intdiv($node['length'], strlen($chunk)) + 1), 0, $node['length']);
    }
    foreach (['$decimal', '$bytes', '$datetime', '$class'] as $marker) {
        if (array_key_exists($marker, $node)) {
            return hydrate($node);
        }
    }
    return array_map('param_node', $node);
}

function html_single(array $case, string $casesDir, string $outDir): array
{
    $document = hydrate($case['document']);
    $parameters = param_node($case['params'] ?? []);
    $options = $case['options'] ?? [];
    $globals = $options['globals'] ?? [];
    unset($options['globals']);
    $dir = $case['template_dir'] === null ? '' : $casesDir . '/' . $case['template_dir'];

    $deprecations = [];
    set_error_handler(static function (int $errno, string $message) use (&$deprecations): bool {
        $deprecations[] = clean($message);
        return true;
    });
    // Se repite si el render cruza un cambio de segundo: así el instante
    // registrado es el que vio date(null).
    for ($attempt = 0; $attempt < 5; $attempt++) {
        $before = time();
        $html = null;
        $error = null;
        try {
            $report = new HtmlReport($dir, $options);
            foreach ($globals as $name => $value) {
                $report->getTwig()->addGlobal($name, $value);
            }
            if ($case['template'] !== null) {
                $report->setTemplate($case['template']);
            }
            $html = $report->render($document, $parameters);
        } catch (Throwable $e) {
            $error = throwable_info($e);
        }
        if (time() === $before) {
            break;
        }
    }
    restore_error_handler();
    $result = ['now' => $before];
    if ($error !== null) {
        $result['error'] = $error;
    } else {
        // Las salidas de más de 1 MiB (logo grande) solo se registran por hash.
        if (strlen($html) <= 1024 * 1024) {
            file_put_contents($outDir . '/' . $case['id'] . '.html', $html);
        } else {
            $result['stored'] = false;
        }
        $result['sha256'] = hash('sha256', $html);
        $result['bytes'] = strlen($html);
    }
    $result['notices'] = array_values(array_unique($deprecations));
    return $result;
}

function read_cases(string $file): array
{
    return json_decode((string) file_get_contents($file), true, 512, JSON_THROW_ON_ERROR)['cases'];
}

function run_html(string $casesFile, string $casesDir, string $outDir): array
{
    $results = [];
    foreach (read_cases($casesFile) as $case) {
        $id = $case['id'];
        if (isset($results[$id]) || !preg_match('/^[a-z0-9-]+$/', $id)) {
            fail("id inválido o duplicado: $id");
        }
        // Un proceso por caso: Twig guarda en memoria las plantillas
        // compiladas y su clave no incluye todas las opciones.
        $cmd = [PHP_BINARY, '-n', __FILE__, '--single', $casesFile, $casesDir, $outDir, $id];
        $proc = proc_open($cmd, [1 => ['pipe', 'w'], 2 => ['pipe', 'w']], $pipes);
        $stdout = stream_get_contents($pipes[1]);
        $stderr = stream_get_contents($pipes[2]);
        $status = proc_close($proc);
        if ($status !== 0) {
            fail("caso $id: estado $status\n$stderr");
        }
        $results[$id] = json_decode((string) $stdout, true, 512, JSON_THROW_ON_ERROR);
    }
    return $results;
}

function run_qr(string $casesFile, string $outDir): array
{
    $writer = new Writer(new ImageRenderer(new RendererStyle(120, 0), new SvgImageBackEnd()));
    $results = [];
    foreach (read_cases($casesFile) as $case) {
        $content = base64_decode($case['content']['$bytes'], true);
        $result = [];
        try {
            $code = Encoder::encode($content, ErrorCorrectionLevel::Q(), 'UTF-8');
            $matrix = $code->getMatrix();
            $rows = [];
            for ($y = 0; $y < $matrix->getHeight(); $y++) {
                $row = '';
                for ($x = 0; $x < $matrix->getWidth(); $x++) {
                    $row .= $matrix->get($x, $y) === 1 ? '1' : '0';
                }
                $rows[] = $row;
            }
            $result['mode'] = (string) $code->getMode();
            $result['version'] = $code->getVersion()->getVersionNumber();
            $result['mask'] = $code->getMaskPattern();
            $result['rows'] = $rows;
        } catch (Throwable $e) {
            $result['encode_error'] = throwable_info($e);
        }
        try {
            $svg = $writer->writeString($content, 'UTF-8', ErrorCorrectionLevel::Q());
            file_put_contents($outDir . '/' . $case['id'] . '.svg', $svg);
            $result['svg_sha256'] = hash('sha256', $svg);
        } catch (Throwable $e) {
            $result['error'] = throwable_info($e);
        }
        $results[$case['id']] = $result;
    }
    return $results;
}

function hash_call(callable $call): array
{
    try {
        set_error_handler(static fn (): bool => true);
        return ['hash' => $call()];
    } catch (Throwable $e) {
        return ['error' => throwable_info($e)];
    } finally {
        restore_error_handler();
    }
}

function run_xml(string $casesFile, string $outDir): array
{
    $utils = new XmlUtils();
    $results = [];
    foreach (read_cases($casesFile) as $case) {
        $id = $case['id'];
        if (isset($case['file'])) {
            $path = $outDir . '/' . $case['file'];
            $results[$id] = ['file' => hash_call(static fn () => $utils->getHashSignFromFile($path))];
            continue;
        }
        $xml = base64_decode($case['xml']['$bytes'], true);
        $path = $outDir . '/' . $id . '.xml';
        file_put_contents($path, $xml);
        $results[$id] = [
            'string' => hash_call(static fn () => $utils->getHashSign($xml)),
            'file' => hash_call(static fn () => $utils->getHashSignFromFile($path)),
            'doc' => hash_call(static function () use ($utils, $xml) {
                $doc = new DOMDocument();
                @$doc->loadXML($xml);
                return $utils->getHashSignFromDoc($doc);
            }),
        ];
        unlink($path);
    }
    return $results;
}

function run_resolver(string $casesFile): array
{
    $resolver = new DefaultTemplateResolver();
    $map = class_map();
    $results = [];
    foreach (read_cases($casesFile) as $case) {
        try {
            $results[$case['id']] = ['template' => $resolver->getTemplate(new ($map[$case['class']])())];
        } catch (Throwable $e) {
            $results[$case['id']] = ['error' => throwable_info($e)];
        }
    }
    return $results;
}

function run_catalog(): array
{
    $filter = new DocumentFilter();
    $store = (new ReflectionProperty(DocumentFilter::class, 'store'))->getValue($filter);
    $entries = [];
    foreach ($store as $code => $values) {
        foreach ($values as $key => $text) {
            $entries[] = ['catalog' => (string) $code, 'key' => (string) $key, 'value' => $filter->getValueCatalog((string) $key, (string) $code)];
        }
    }
    $misses = [];
    foreach ([['01', '50'], ['011', '01'], ['021', 'GB'], ['022', 'USD'], ['', ''], ['06', '']] as [$code, $key]) {
        $misses[] = ['catalog' => $code, 'key' => $key, 'value' => $filter->getValueCatalog($key, $code)];
    }
    return ['entries' => $entries, 'misses' => $misses];
}

function envelope(array $results): string
{
    return json_encode([
        'reference_version' => PHP_VERSION,
        'timezone_db' => timezone_version_get(),
        'libxml' => LIBXML_DOTTED_VERSION,
        'results' => $results,
    ], JSON_FLAGS | JSON_PRETTY_PRINT) . "\n";
}

$mode = $argv[1] ?? '';
if ($mode === '--single') {
    [, , $casesFile, $casesDir, $outDir, $id] = $argv + array_fill(0, 7, null);
    $casesRoot = realpath($casesDir) ?: $casesDir;
    foreach (read_cases($casesFile) as $case) {
        if ($case['id'] === $id) {
            echo json_encode(html_single($case, $casesRoot, $outDir), JSON_FLAGS);
            exit(0);
        }
    }
    fail("caso no encontrado: $id");
}

switch ($mode) {
    case 'html':
        [, , $casesFile, $casesDir, $outDir] = $argv + array_fill(0, 5, null);
        if (!is_dir($outDir) && !mkdir($outDir, 0777, true)) {
            fail("no se pudo crear $outDir");
        }
        echo envelope(run_html($casesFile, realpath($casesDir), $outDir));
        break;
    case 'qr':
        [, , $casesFile, $outDir] = $argv + array_fill(0, 4, null);
        if (!is_dir($outDir) && !mkdir($outDir, 0777, true)) {
            fail("no se pudo crear $outDir");
        }
        echo envelope(run_qr($casesFile, $outDir));
        break;
    case 'xml':
        [, , $casesFile, $outDir] = $argv + array_fill(0, 4, null);
        if (!is_dir($outDir) && !mkdir($outDir, 0777, true)) {
            fail("no se pudo crear $outDir");
        }
        echo envelope(run_xml($casesFile, $outDir));
        break;
    case 'resolver':
        echo envelope(run_resolver($argv[2] ?? fail('falta casos.json')));
        break;
    case 'catalog':
        echo envelope(run_catalog());
        break;
    default:
        fail('modo desconocido; ver la cabecera del script');
}
