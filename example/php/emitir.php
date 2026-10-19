<?php

declare(strict_types=1);

/*
 * Genera y firma los casos de casos/ con Greenter PHP.
 *
 *   php emitir.php                    # todos los casos: XML firmado en out/
 *   php emitir.php factura boleta     # solo esos casos
 *   php emitir.php --enviar [casos]   # además los envía a los entornos de prueba
 *   php emitir.php --errores [casos]  # rechazos en out/errores/
 *
 * Cada caso es casos/<nombre>.php y devuelve una función que recibe el
 * Contexto y construye el documento (equivale a example/rust/src/casos/).
 *
 * EJEMPLO_FECHA ("AAAA-MM-DD HH:MM:SS", hora de Lima) fija la fecha base;
 * sin ella se usa la hora actual. Con la misma fecha, el XML firmado debe ser
 * idéntico al del ejemplo Rust (oracle/ejemplos/comparar.sh).
 *
 * Envío según el documento:
 * - factura, boleta, nota: sendBill a SUNAT BETA y CDR;
 * - percepción, retención: sendBill al servicio BETA de retenciones;
 * - resumen, baja: sendSummary a SUNAT BETA, ticket y getStatus;
 * - reversión: igual, en el servicio BETA de retenciones;
 * - guía: API GRE de prueba (GRE_API_URL), ticket y estado.
 * SUNAT BETA responde 401 a los envíos muy seguidos: entre uno y otro se
 * esperan EJEMPLO_PAUSA segundos (5 por defecto).
 *
 * Benchmark: cada fase de cada caso se mide y queda en out/benchmark.json,
 * que oracle/ejemplos/benchmark.py compara con el del ejemplo Rust:
 * - construir (el modelo), xml (el builder), firma (SignedXml) y see
 *   (See::getXmlSigned completo): mediana de EJEMPLO_REPETICIONES llamadas
 *   (10 por defecto) en microsegundos, y la primera llamada de `see`;
 * - envio (See/Api::send) y estado (las consultas de ticket, sin las pausas):
 *   una medición en milisegundos, solo con --enviar.
 */

use Greenter\Model\Despatch\Despatch;
use Greenter\Model\DocumentInterface;
use Greenter\Model\Perception\Perception;
use Greenter\Model\Response\BaseResult;
use Greenter\Model\Response\BillResult;
use Greenter\Model\Response\CdrResponse;
use Greenter\Model\Response\StatusResult;
use Greenter\Model\Response\SummaryResult;
use Greenter\Model\Retention\Retention;
use Greenter\Model\Voided\Reversion;

// greenter/xmldsig 5.1.0 emite avisos de deprecación en PHP 8.4 que no
// afectan al resultado; se ocultan para que la salida sea legible.
error_reporting(E_ALL & ~E_DEPRECATED);

require __DIR__.'/vendor/autoload.php';
require __DIR__.'/datos.php';

/** Casos que solo generan XML: su envío original va a un servidor de terceros. */
const SOLO_XML = ['factura-beatose'];

/** @var Config $config */
$config = require __DIR__.'/config.php';
$args = array_slice($argv, 1);
$enviar = in_array('--enviar', $args, true);
$errores = in_array('--errores', $args, true);
$filtro = array_values(array_filter($args, static fn (string $a): bool => $a !== '--enviar' && $a !== '--errores'));

$lima = new DateTimeZone('America/Lima');
$texto = getenv('EJEMPLO_FECHA');
$fecha = $texto
    ? DateTime::createFromFormat('!Y-m-d H:i:s', $texto, $lima)
    : new DateTime('now', $lima);
if ($fecha === false) {
    fwrite(STDERR, "EJEMPLO_FECHA debe tener el formato AAAA-MM-DD HH:MM:SS\n");
    exit(2);
}
$ctx = new Contexto($fecha, $config->ruc(), $config->greRuc());

$casos = [];
$directorioCasos = $errores ? 'errores' : 'casos';
foreach (glob(__DIR__."/$directorioCasos/*.php") as $archivo) {
    $casos[basename($archivo, '.php')] = $archivo;
}
ksort($casos, SORT_STRING);
foreach ($filtro as $nombre) {
    if (!isset($casos[$nombre])) {
        fwrite(STDERR, "Caso desconocido: $nombre\n");
        exit(2);
    }
}

$out = __DIR__.'/out'.($errores ? '/errores' : '');
if (!is_dir($out) && !mkdir($out, 0775, true) && !is_dir($out)) {
    throw new RuntimeException("No se pudo crear $out");
}

$see = $config->see();
$pausa = max(0, (int) (getenv('EJEMPLO_PAUSA') ?: 5));
$repeticiones = max(1, (int) (getenv('EJEMPLO_REPETICIONES') ?: 10));
$enviados = 0;
$fallos = 0;
$nombres = [];
$benchmark = [];

foreach ($casos as $caso => $archivo) {
    if ($filtro && !in_array($caso, $filtro, true)) {
        continue;
    }
    $construir = require $archivo;
    /** @var DocumentInterface $doc */
    [$doc, $tConstruir] = medir(static fn () => $construir($ctx), $repeticiones);
    $name = $doc->getName();
    if (isset($nombres[$name])) {
        throw new RuntimeException("$caso y {$nombres[$name]} generan el mismo archivo $name");
    }
    $nombres[$name] = $caso;
    [$xml, $tSee, $tSeePrimera] = medir(static fn () => $see->getXmlSigned($doc), $repeticiones);
    // Las fases por separado, con el builder y el firmador que See dejó listos.
    $builder = $see->getFactory()->getBuilder();
    $signer = $see->getFactory()->getSigner();
    [$sinFirma, $tXml] = medir(static fn () => $builder->build($doc), $repeticiones);
    [$firmado, $tFirma] = medir(static fn () => $signer->signXml($sinFirma), $repeticiones);
    if ($firmado !== $xml) {
        throw new RuntimeException("$caso: builder + firma no reproducen getXmlSigned");
    }
    file_put_contents("$out/$name.xml", $xml);
    echo "$caso: $name.xml (".strlen($xml)." bytes)".PHP_EOL;
    printf("  tiempos: construir %s · xml %s · firma %s · getXmlSigned %s (primera %s)%s",
        us($tConstruir), us($tXml), us($tFirma), us($tSee), us($tSeePrimera), PHP_EOL);
    $medida = [
        'caso' => $caso,
        'archivo' => "$name.xml",
        'construir_us' => $tConstruir,
        'xml_us' => $tXml,
        'firma_us' => $tFirma,
        'see_us' => $tSee,
        'see_primera_us' => $tSeePrimera,
        'envio_ms' => null,
        'estado_ms' => null,
        'resultado' => null,
        'codigo' => null,
        'mensaje' => null,
        'observaciones' => [],
    ];

    if ($enviar && in_array($caso, SOLO_XML, true)) {
        echo '  sin envío: el demo lo envía a un servidor de terceros'.PHP_EOL;
    } elseif ($enviar) {
        if ($enviados++ > 0) {
            sleep($pausa);
        }
        $aceptado = enviar($config, $doc, $xml, $out, $pausa, $medida);
        if ($aceptado === $errores) {
            $fallos++;
        }
        printf("  tiempos de red: envío %.0f ms · estado %s%s", $medida['envio_ms'],
            $medida['estado_ms'] === null ? '-' : sprintf('%.0f ms', $medida['estado_ms']), PHP_EOL);
    }
    $benchmark[] = $medida;
}

file_put_contents("$out/benchmark.json", json_encode([
    'lenguaje' => 'php',
    'version' => PHP_VERSION,
    'opcache' => function_exists('opcache_get_status') && (bool) ini_get('opcache.enable_cli'),
    'jit' => (string) ini_get('opcache.jit'),
    'repeticiones' => $repeticiones,
    'enviar' => $enviar,
    'errores' => $errores,
    'casos' => $benchmark,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_PRESERVE_ZERO_FRACTION).PHP_EOL);
printf("Total (medianas): construir %s · xml %s · firma %s · getXmlSigned %s%s",
    us(array_sum(array_column($benchmark, 'construir_us'))), us(array_sum(array_column($benchmark, 'xml_us'))),
    us(array_sum(array_column($benchmark, 'firma_us'))), us(array_sum(array_column($benchmark, 'see_us'))), PHP_EOL);

exit($fallos > 0 ? 1 : 0);

/**
 * Llama a $f $veces y devuelve [último resultado, mediana µs, primera µs].
 *
 * @return array{0: mixed, 1: float, 2: float}
 */
function medir(callable $f, int $veces): array
{
    $tiempos = [];
    $resultado = null;
    for ($i = 0; $i < $veces; $i++) {
        $inicio = hrtime(true);
        $resultado = $f();
        $tiempos[] = (hrtime(true) - $inicio) / 1000;
    }
    $primera = $tiempos[0];
    sort($tiempos);
    $n = count($tiempos);
    $mediana = $n % 2 === 1 ? $tiempos[intdiv($n, 2)] : ($tiempos[$n / 2 - 1] + $tiempos[$n / 2]) / 2;

    return [$resultado, $mediana, $primera];
}

/** Microsegundos legibles: «850 µs», «12.4 ms». */
function us(float $us): string
{
    return $us < 1000 ? sprintf('%.0f µs', $us) : sprintf('%.1f ms', $us / 1000);
}

/** Envía según el tipo de documento e informa el resultado. */
function enviar(Config $config, DocumentInterface $doc, string $xml, string $out, int $pausa, array &$medida): bool
{
    $name = $doc->getName();
    $medida['envio_ms'] = 0.0;
    if ($doc instanceof Despatch) {
        $api = $config->api();
        if ($api === null) {
            echo '  sin envío: faltan GRE_* en el archivo de credenciales'.PHP_EOL;
            $medida['resultado'] = 'sin envío';

            return true;
        }
        $inicio = hrtime(true);
        $result = $api->send($doc);
        $medida['envio_ms'] = (hrtime(true) - $inicio) / 1e6;
        $enviado = $api->getLastXml();
        $estado = static fn (string $ticket): StatusResult => $api->getStatus($ticket);
    } else {
        $retencion = $doc instanceof Perception || $doc instanceof Retention || $doc instanceof Reversion;
        $see = $retencion ? $config->seeRetencion() : $config->see();
        $inicio = hrtime(true);
        $result = $see->send($doc);
        $medida['envio_ms'] = (hrtime(true) - $inicio) / 1e6;
        $enviado = $see->getFactory()->getLastXml();
        $estado = static fn (string $ticket): StatusResult => $see->getStatus($ticket);
    }
    if ($enviado !== $xml) {
        echo '  aviso: el XML enviado difiere del generado antes'.PHP_EOL;
    }
    if (!$result->isSuccess()) {
        return error($result, $medida);
    }
    if ($result instanceof BillResult) {
        file_put_contents("$out/R-$name.zip", $result->getCdrZip());

        return cdr($result->getCdrResponse(), $medida);
    }

    /** @var SummaryResult $result */
    $ticket = (string) $result->getTicket();
    echo "  ticket: $ticket".PHP_EOL;
    // El ticket puede quedar «en proceso» (98) unos segundos.
    $medida['estado_ms'] = 0.0;
    for ($intento = 1; $intento <= 5; $intento++) {
        sleep($pausa);
        $inicio = hrtime(true);
        $status = $estado($ticket);
        $medida['estado_ms'] += (hrtime(true) - $inicio) / 1e6;
        if ($status->getCode() !== '98') {
            break;
        }
        echo '  en proceso…'.PHP_EOL;
    }
    if (!$status->isSuccess()) {
        return error($status, $medida);
    }
    file_put_contents("$out/R-$name.zip", $status->getCdrZip());

    return cdr($status->getCdrResponse(), $medida);
}

function error(BaseResult $result, array &$medida): bool
{
    $error = $result->getError();
    echo "  ERROR {$error->getCode()}: {$error->getMessage()}".PHP_EOL;
    $medida['resultado'] = "error {$error->getCode()}";
    $medida['codigo'] = (string) $error->getCode();
    $medida['mensaje'] = (string) $error->getMessage();

    return false;
}

/** Interpreta el CDR como en https://greenter.dev/starter/. */
function cdr(?CdrResponse $cdr, array &$medida): bool
{
    $medida['codigo'] = $cdr?->getCode();
    $medida['mensaje'] = $cdr?->getDescription();
    $medida['observaciones'] = $cdr?->getNotes() ?? [];
    $code = (int) $cdr?->getCode();
    $ok = $code === 0;
    if ($ok) {
        $medida['resultado'] = 'aceptada';
    } elseif ($code >= 2000 && $code <= 3999) {
        $medida['resultado'] = 'rechazada';
    } else {
        $medida['resultado'] = 'excepción';
    }
    echo '  ESTADO: '.strtoupper(str_replace('ó', 'O', $medida['resultado'])).PHP_EOL;
    echo "  {$cdr?->getDescription()}".PHP_EOL;
    foreach ($cdr?->getNotes() ?? [] as $nota) {
        echo "  observación: $nota".PHP_EOL;
    }

    return $ok;
}
