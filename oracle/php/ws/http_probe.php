<?php
declare(strict_types=1);

// BillSender con el SoapClient real de Greenter v5.3.0 (ext/soap) contra el
// servidor HTTP de oracle/ws/http_status_server.py. Solo destinos loopback.
ini_set('display_errors', '0');
error_reporting(E_ALL);

$root = dirname(__DIR__, 3);
$reference = $root.'/target/reference/v5.3.0';
$base = getenv('WS_HTTP_URL') ?: '';
if (!preg_match('~^http://127\.0\.0\.1:[0-9]+$~', $base)) {
    throw new RuntimeException('Solo HTTP loopback');
}
if (trim((string) shell_exec('git -C '.escapeshellarg($reference).' rev-parse HEAD')) !== 'f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f') {
    throw new RuntimeException('Referencia PHP incorrecta');
}
require $root.'/target/reference/v5.3.0-suite/vendor/autoload.php';
spl_autoload_register(static function (string $class) use ($reference): void {
    if (!str_starts_with($class, 'Greenter\\')) return;
    $relative = substr($class, strlen('Greenter\\'));
    foreach (['ws/src', 'core/src/Core'] as $package) {
        $path = $reference.'/packages/'.$package.'/'.str_replace('\\', '/', $relative).'.php';
        if (is_file($path)) { require $path; return; }
    }
}, true, true);

use Greenter\Ws\Services\BillSender;
use Greenter\Ws\Services\SummarySender;
use Greenter\Ws\Services\SoapClient;

/**
 * Perfil de libxml2 detectado por su comportamiento: la 2.9 (Ubuntu) une dos
 * secciones CDATA contiguas en un nodo y las versiones recientes no. Cambia
 * el resultado de un faultstring con varias CDATA (caso fault-cdata-two;
 * tests/reference/ws/differences.json, only_when).
 */
function libxml_cdata(): string
{
    $doc = new DOMDocument();
    $doc->loadXML('<a><![CDATA[a]]><![CDATA[b]]></a>');

    return $doc->documentElement->childNodes->length === 1 ? 'unidos' : 'separados';
}

$ids = array_slice($argv, 1);
$out = [];
foreach ($ids as $id) {
    $client = new SoapClient();
    $client->setCredentials('USUARIO-FICTICIO', 'CLAVE-FICTICIA');
    $client->setService("$base/$id");
    // Los casos `summary-*` usan SummarySender; el resto, BillSender.
    $summary = str_starts_with($id, 'summary-');
    $sender = $summary ? new SummarySender() : new BillSender();
    $sender->setClient($client);
    try {
        $result = $sender->send('20000000001-01-F001-1', '<Invoice/>');
    } catch (Throwable $e) {
        // La excepción escapa de BillSender (p. ej. TypeError con un fault
        // sin faultcode); en Rust es un Err.
        $out[] = ['id' => $id, 'exception' => true];
        continue;
    }
    $error = $result->getError();
    if ($summary) {
        $out[] = [
            'id' => $id,
            'success' => $result->isSuccess(),
            'error' => $error ? ['code' => $error->getCode(), 'message' => $error->getMessage()] : null,
            'ticket' => $result->getTicket(),
        ];
        continue;
    }
    $cdr = $result->getCdrResponse();
    $out[] = [
        'id' => $id,
        'success' => $result->isSuccess(),
        'error' => $error ? ['code' => $error->getCode(), 'message' => $error->getMessage()] : null,
        'cdr' => $cdr ? [
            'id' => $cdr->getId(),
            'code' => $cdr->getCode(),
            'description' => $cdr->getDescription(),
            'notes' => $cdr->getNotes(),
        ] : null,
    ];
}
echo json_encode(['libxml' => LIBXML_DOTTED_VERSION, 'libxml_cdata' => libxml_cdata(), 'cases' => $out], JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
