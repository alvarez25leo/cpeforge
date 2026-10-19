<?php

declare(strict_types=1);

/*
 * Oráculo PHP de greenter/xml v5.3.0.
 *
 * Para cada caso construye el modelo de greenter/core con los setters reales
 * (misma gramática que oracle/php/core/run_cases.php), instancia el builder
 * PHP indicado con las opciones de Twig del caso y guarda el XML devuelto
 * **sin modificar** en <salida>/<id>.xml. El resumen JSON registra el
 * instante de render (la plantilla usa la hora actual cuando una fecha es
 * nula), el error si lo hubo y los avisos de deprecación emitidos por PHP.
 *
 * Uso: php oracle/php/xml/run_cases.php <casos.json> <directorio_salida>
 *
 * Cada caso se ejecuta en un subproceso `php -n` (sin php.ini local).
 */

require dirname(__DIR__) . '/core/lib.php';

$vendor = getenv('GREENTER_PHP_ORACLE_VENDOR') ?: dirname(__DIR__, 3) . '/target/php-oracle-vendor';
if (!is_file($vendor . '/autoload.php')) {
    fail("Falta $vendor/autoload.php; ejecutar composer install en oracle/php");
}
require $vendor . '/autoload.php';

$xmlSrc = realpath(GREENTER_CORE_SRC . '/../../../xml/src');
if ($xmlSrc === false) {
    fail('No se encontró packages/xml/src en la referencia');
}
define('GREENTER_XML_SRC', $xmlSrc);
spl_autoload_register(static function (string $class) use ($xmlSrc): void {
    if (strncmp($class, 'Greenter\\Xml\\', 13) !== 0) {
        return;
    }
    $file = $xmlSrc . '/' . str_replace('\\', '/', substr($class, strlen('Greenter\\'))) . '.php';
    if (is_file($file)) {
        require $file;
    }
});

// Valores por defecto de PHP, fijados para no depender del php.ini local.
ini_set('precision', '14');
ini_set('serialize_precision', '-1');
date_default_timezone_set('UTC');

/** @var list<string> */
$deprecations = [];
set_error_handler(static function (int $errno, string $message) use (&$deprecations): bool {
    if ($errno === E_DEPRECATED || $errno === E_USER_DEPRECATED) {
        $deprecations[] = sanitize($message);
        return true;
    }
    return false;
});

const BUILDERS = [
    'InvoiceBuilder', 'NoteBuilder', 'DespatchBuilder', 'SummaryBuilder',
    'VoidedBuilder', 'PerceptionBuilder', 'RetentionBuilder',
];

/** Elimina rutas absolutas de mensajes de error para que el fixture sea estable. */
function sanitize(string $message): string
{
    $message = preg_replace('/, called in \S+ on line \d+/', '', $message);
    $message = str_replace(GREENTER_XML_SRC, '<xml-src>', $message);
    return str_replace(dirname(GREENTER_CORE_SRC, 4), '<ref>', $message);
}

function throwable_info(Throwable $e): array
{
    $info = ['class' => get_class($e), 'message' => sanitize($e->getMessage())];
    if ($e->getPrevious() !== null) {
        $info['previous'] = throwable_info($e->getPrevious());
    }
    return $info;
}

/**
 * Ejecuta un caso y devuelve su resumen. Cada caso se ejecuta en un proceso
 * PHP propio (ver más abajo): Twig guarda en memoria la clase compilada de
 * cada plantilla y su clave no incluye `autoescape`, así que casos con
 * opciones distintas en un mismo proceso contaminarían la salida.
 */
function run_single(array $case, string $outDir, array &$deprecations): array
{
    $id = $case['id'];
    $options = $case['options'] ?? [];
    $class = 'Greenter\\Xml\\Builder\\' . $case['builder'];
    $document = hydrate($case['input']);

    // Se repite el render si cruza un cambio de segundo: así el instante
    // registrado es exactamente el que vio la plantilla. Los avisos se
    // acumulan entre intentos (Twig solo avisa al compilar la primera vez).
    $deprecations = [];
    for ($attempt = 0; $attempt < 5; $attempt++) {
        $before = time();
        $xml = null;
        $error = null;
        try {
            $builder = new $class($options);
            $xml = $builder->build($document);
        } catch (Throwable $e) {
            $error = throwable_info($e);
        }
        if (time() === $before) {
            break;
        }
    }
    $result = ['now' => $before];
    if ($error !== null) {
        $result['error'] = $error;
    } elseif ($xml === null) {
        $result['null'] = true;
    } else {
        file_put_contents("$outDir/$id.xml", $xml);
        $result['sha256'] = hash('sha256', $xml);
        $result['bytes'] = strlen($xml);
    }
    $result['deprecations'] = array_values(array_unique($deprecations));
    return $result;
}

if (($argv[1] ?? '') === '--single') {
    [, , $casesFile, $outDir, $id] = $argv + [null, null, null, null, null];
    $cases = json_decode((string) file_get_contents($casesFile), true, 512, JSON_THROW_ON_ERROR);
    foreach ($cases['cases'] as $case) {
        if ($case['id'] === $id) {
            echo json_encode(run_single($case, $outDir, $deprecations), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR);
            exit(0);
        }
    }
    fail("caso no encontrado: $id");
}

$casesFile = $argv[1] ?? fail('uso: run_cases.php <casos.json> <salida>');
$outDir = $argv[2] ?? fail('uso: run_cases.php <casos.json> <salida>');
if (!is_dir($outDir) && !mkdir($outDir, 0777, true)) {
    fail("no se pudo crear $outDir");
}
$cases = json_decode((string) file_get_contents($casesFile), true, 512, JSON_THROW_ON_ERROR);

$results = [];
foreach ($cases['cases'] as $case) {
    $id = $case['id'];
    if (isset($results[$id]) || !preg_match('/^[a-z0-9-]+$/', $id)) {
        fail("id inválido o duplicado: $id");
    }
    if (!in_array($case['builder'], BUILDERS, true)) {
        fail("builder desconocido: {$case['builder']}");
    }
    $command = [PHP_BINARY, '-n', '-d', 'memory_limit=256M', __FILE__, '--single', $casesFile, $outDir, $id];
    $process = proc_open($command, [1 => ['pipe', 'w'], 2 => ['pipe', 'w']], $pipes);
    if (!is_resource($process)) {
        fail("no se pudo ejecutar el caso $id");
    }
    $stdout = stream_get_contents($pipes[1]);
    $stderr = stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    if (proc_close($process) !== 0) {
        fail("el caso $id falló: $stderr");
    }
    $results[$id] = json_decode((string) $stdout, true, 512, JSON_THROW_ON_ERROR);
}

$reference = json_decode((string) file_get_contents(dirname(__DIR__, 3) . '/oracle/upstream/referencia.json'), true);
$lock = json_decode((string) file_get_contents(dirname(__DIR__) . '/composer.lock'), true);
$twig = array_values(array_filter($lock['packages'], static fn ($p) => $p['name'] === 'twig/twig'))[0]['version'] ?? null;
echo json_encode([
    'generator' => 'oracle/php/xml/run_cases.php',
    'reference' => ['tag' => $reference['baseline_tag'], 'commit' => $reference['baseline_commit']],
    'reference_version' => PHP_VERSION,
    'twig_version' => $twig,
    'timezone_db' => timezone_version_get(),
    'cases_file' => basename($casesFile),
    'results' => $results,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
