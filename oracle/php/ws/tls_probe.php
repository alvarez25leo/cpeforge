<?php
declare(strict_types=1);

// El único destino permitido es un servidor HTTPS efímero en loopback.
$root = dirname(__DIR__, 3);
$reference = $root.'/target/reference/v5.3.0';
$url = getenv('WS_TLS_URL') ?: '';
if (!preg_match('~^https://127\.0\.0\.1:[0-9]+/$~', $url)) {
    throw new RuntimeException('Solo HTTPS loopback');
}
if (trim((string) shell_exec('git -C '.escapeshellarg($reference).' rev-parse HEAD')) !== 'f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f') {
    throw new RuntimeException('Referencia PHP incorrecta');
}
require $root.'/target/reference/v5.3.0-suite/vendor/autoload.php';
spl_autoload_register(static function (string $class) use ($reference): void {
    if (!str_starts_with($class, 'Greenter\\Ws\\')) return;
    $path = $reference.'/packages/ws/src/'.str_replace('\\', '/', substr($class, strlen('Greenter\\'))).'.php';
    if (is_file($path)) require $path;
}, true, true);

$client = new Greenter\Ws\Services\SoapClient();
$client->setCredentials('USUARIO-FICTICIO', 'CLAVE-FICTICIA');
$client->setService($url);
$response = $client->call('getStatus', ['parameters' => ['ticket' => 'TICKET-FICTICIO']]);
echo json_encode(['code' => $response->status->statusCode], JSON_THROW_ON_ERROR), "\n";
