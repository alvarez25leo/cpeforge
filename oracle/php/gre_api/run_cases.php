<?php

declare(strict_types=1);

/*
 * Oráculo PHP de greenter/gre-api v1.0.2 (código real del checkout aislado)
 * con Guzzle 7.15.5, contra el servidor local oracle/gre_api/mock_server.py.
 *
 * Uso: php oracle/php/gre_api/run_cases.php <cases.json> <url_base_servidor>
 *
 * La salida contiene, por caso, el resultado o la excepción y las
 * solicitudes que registró el servidor (con su validación de contrato).
 */

$root = dirname(__DIR__, 3);
$refDir = getenv('GRE_API_PHP_REFERENCE') ?: $root . '/target/reference/gre-api-v1.0.2';
require $root . '/target/php-oracle-vendor/autoload.php';
spl_autoload_register(static function (string $class) use ($refDir): void {
    $prefix = 'Greenter\\Sunat\\GRE\\';
    if (strncmp($class, $prefix, strlen($prefix)) === 0) {
        $file = $refDir . '/src/' . str_replace('\\', '/', substr($class, strlen($prefix))) . '.php';
        if (is_file($file)) {
            require $file;
        }
    }
});

use Greenter\Sunat\GRE\Api\AuthApi;
use Greenter\Sunat\GRE\Api\CpeApi;
use Greenter\Sunat\GRE\ApiException;
use Greenter\Sunat\GRE\Configuration;
use Greenter\Sunat\GRE\Model\CpeDocument;
use Greenter\Sunat\GRE\Model\CpeDocumentArchivo;
use Greenter\Sunat\GRE\Model\ModelInterface;
use GuzzleHttp\Client;

date_default_timezone_set('UTC');
ini_set('precision', '14');

$cases = json_decode((string) file_get_contents($argv[1]), true, 512, JSON_THROW_ON_ERROR);
$base = rtrim($argv[2], '/');
if (!preg_match('~^http://127\.0\.0\.1:[0-9]+$~', $base)) {
    throw new LogicException('El oráculo solo permite un servidor en 127.0.0.1');
}

$warnings = [];
set_error_handler(static function (int $no, string $message) use (&$warnings): bool {
    $warnings[] = $message;
    return true;
});

function arg_value($v)
{
    if (is_array($v) && isset($v['repeat'])) {
        return str_repeat($v['repeat']['unit'], $v['repeat']['count']);
    }
    return $v;
}

function bytes_out(?string $s)
{
    if ($s === null) {
        return null;
    }
    return mb_check_encoding($s, 'UTF-8') ? $s : ['b64' => base64_encode($s)];
}

function headers_out(?array $headers): ?array
{
    if ($headers === null) {
        return null;
    }
    $out = [];
    foreach ($headers as $name => $values) {
        $out[] = [strtolower((string) $name), array_values($values)];
    }
    return $out;
}

function dump($value)
{
    if ($value instanceof ModelInterface) {
        $out = ['_model' => $value->getModelName()];
        foreach ($value::openAPITypes() as $property => $type) {
            $getter = $value::getters()[$property];
            $out[$value::attributeMap()[$property]] = dump($value->$getter());
        }
        return $out;
    }
    if ($value instanceof \DateTime) {
        return $value->format('Y-m-d\TH:i:s.uP');
    }
    if (is_array($value)) {
        return array_map('dump', $value);
    }
    return $value;
}

function error_out(\Throwable $e): array
{
    $out = ['class' => get_class($e), 'message' => $e->getMessage(), 'code' => $e->getCode()];
    if ($e instanceof ApiException) {
        $out['headers'] = headers_out($e->getResponseHeaders());
        $out['body'] = bytes_out($e->getResponseBody());
        $out['object'] = dump($e->getResponseObject());
    }
    return $out;
}

function request_out(\Psr\Http\Message\RequestInterface $r): array
{
    $headers = [];
    foreach ($r->getHeaders() as $name => $values) {
        if (strcasecmp($name, 'Host') !== 0) { // lo añade PSR-7 desde la URI; el transporte Rust lo envía igual
            $headers[] = [$name, implode(', ', $values)];
        }
    }
    return ['method' => $r->getMethod(), 'url' => (string) $r->getUri(), 'headers' => $headers, 'body' => bytes_out((string) $r->getBody())];
}

function document($d): ?CpeDocument
{
    if ($d === null) {
        return null;
    }
    $data = [];
    if (array_key_exists('archivo', $d) && $d['archivo'] !== null) {
        $data['archivo'] = new CpeDocumentArchivo(array_map('arg_value', $d['archivo']));
    }
    return new CpeDocument($data);
}

function config_for(array $c, string $base): Configuration
{
    $cfg = new Configuration();
    $host = $c['config']['host'] ?? null;
    if ($host === 'mock') {
        $cfg->setHost($base . '/c/' . $c['id']);
    } elseif ($host !== null) {
        $cfg->setHost($host);
    }
    if (array_key_exists('access_token', $c['config'])) {
        $cfg->setAccessToken($c['config']['access_token']);
    }
    if (array_key_exists('user_agent', $c['config'])) {
        $cfg->setUserAgent($c['config']['user_agent']);
    }
    return $cfg;
}

function run_case(array $c, string $base)
{
    if ($c['operation'] === 'config') {
        $cfg = new Configuration();
        $hosts = [];
        foreach ([0, 1, 2, -1] as $i) {
            try {
                $hosts[] = ['index' => $i, 'host' => $cfg->getHostFromSettings($i)];
            } catch (\Throwable $e) {
                $hosts[] = ['index' => $i, 'error' => error_out($e)];
            }
        }
        return [
            'host' => $cfg->getHost(), 'user_agent' => $cfg->getUserAgent(), 'access_token' => $cfg->getAccessToken(),
            'host_settings' => $cfg->getHostSettings(), 'host_from_settings' => $hosts,
            'cpe_api_host' => (new CpeApi(null, $cfg))->getConfig()->getHost(),
        ];
    }
    // Puerta de red del oráculo: solo se permite conectar al servidor local
    // o al puerto 1 local del caso de conexión rechazada. Las llamadas
    // «request» construyen la solicitud, pero nunca se envían.
    $host = $c['config']['host'] ?? null;
    if (!in_array($c['variant'], ['request', 'config'], true)
        && $host !== 'mock'
        && $host !== 'http://127.0.0.1:1') {
        throw new LogicException('El oráculo solo permite destinos locales');
    }
    $options = $c['client'] ?? [];
    $options['allow_redirects'] = [
        'max' => 5,
        'on_redirect' => static function ($request, $response, $uri): void {
            if (!in_array(strtolower($uri->getHost()), ['127.0.0.1', 'localhost'], true)) {
                throw new LogicException('Redirección fuera del servidor local bloqueada');
            }
        },
    ];
    $client = new Client($options);
    $cfg = config_for($c, $base);
    $ct = $c['content_type'] ?? null;
    $a = $c['args'];
    $v = $c['variant'];
    if ($c['operation'] === 'getToken') {
        $api = new AuthApi($client, $cfg);
        $p = [$a['grant_type'], $a['scope'], $a['client_id'], $a['client_secret'], $a['username'], $a['password']];
        $ct = $ct ?? AuthApi::contentTypes['getToken'][0];
        switch ($v) {
            case 'sync': return ['data' => dump($api->getToken(...$p))];
            case 'http_info': return info($api->getTokenWithHttpInfo(...[...$p, $ct]));
            case 'async': return ['data' => dump($api->getTokenAsync(...[...$p, $ct])->wait())];
            case 'async_http_info': return info($api->getTokenAsyncWithHttpInfo(...[...$p, $ct])->wait());
            case 'request': return request_out($api->getTokenRequest(...[...$p, $ct]));
        }
    }
    $api = new CpeApi($client, $cfg);
    if ($c['operation'] === 'enviarCpe') {
        $doc = document($a['document']);
        $ct = $ct ?? CpeApi::contentTypes['enviarCpe'][0];
        switch ($v) {
            case 'sync': return ['data' => dump($api->enviarCpe($a['filename'], $doc))];
            case 'http_info': return info($api->enviarCpeWithHttpInfo($a['filename'], $doc, $ct));
            case 'async': return ['data' => dump($api->enviarCpeAsync($a['filename'], $doc, $ct)->wait())];
            case 'async_http_info': return info($api->enviarCpeAsyncWithHttpInfo($a['filename'], $doc, $ct)->wait());
            case 'request': return request_out($api->enviarCpeRequest($a['filename'], $doc, $ct));
        }
    }
    $ct = $ct ?? CpeApi::contentTypes['consultarEnvio'][0];
    switch ($v) {
        case 'sync': return ['data' => dump($api->consultarEnvio($a['ticket']))];
        case 'http_info': return info($api->consultarEnvioWithHttpInfo($a['ticket'], $ct));
        case 'async': return ['data' => dump($api->consultarEnvioAsync($a['ticket'], $ct)->wait())];
        case 'async_http_info': return info($api->consultarEnvioAsyncWithHttpInfo($a['ticket'], $ct)->wait());
        case 'request': return request_out($api->consultarEnvioRequest($a['ticket'], $ct));
    }
    throw new LogicException('variante desconocida');
}

function info(array $r): array
{
    return ['data' => dump($r[0]), 'status' => $r[1], 'headers' => headers_out($r[2])];
}

$results = [];
foreach ($cases['cases'] as $c) {
    $warnings = [];
    try {
        $out = ['result' => run_case($c, $base)];
    } catch (\Throwable $e) {
        $out = ['error' => error_out($e)];
    }
    if (in_array($c['variant'], ['request', 'config'], true) === false && ($c['config']['host'] ?? null) === 'mock') {
        $out['requests'] = json_decode((string) file_get_contents($base . '/__take?case=' . rawurlencode($c['id'])), true);
    }
    $out['warnings'] = array_values(array_unique($warnings));
    $results[$c['id']] = $out;
}
echo json_encode([
    'generator' => 'oracle/php/gre_api/run_cases.php',
    'reference_version' => PHP_VERSION,
    'guzzle' => \GuzzleHttp\ClientInterface::MAJOR_VERSION,
    'curl' => curl_version()['version'],
    'results' => $results,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
