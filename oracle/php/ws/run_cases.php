<?php
declare(strict_types=1);

// Ejecuta exclusivamente código del checkout fijado, con WsClientInterface
// simulado. Ninguna rama de este runner crea un SoapClient real.
ini_set('display_errors', '0');
error_reporting(E_ALL);

$root = dirname(__DIR__, 3);
$reference = getenv('WS_PHP_REFERENCE') ?: $root.'/target/reference/v5.3.0';
$vendor = $root.'/target/reference/v5.3.0-suite/vendor/autoload.php';
if (trim((string) shell_exec('git -C '.escapeshellarg($reference).' rev-parse HEAD')) !== 'f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f') {
    throw new RuntimeException('Referencia PHP incorrecta');
}
require $vendor;
spl_autoload_register(static function (string $class) use ($reference): void {
    if (!str_starts_with($class, 'Greenter\\')) return;
    $relative = substr($class, strlen('Greenter\\'));
    foreach (['ws/src', 'core/src/Core'] as $package) {
        $path = $reference.'/packages/'.$package.'/'.str_replace('\\', '/', $relative).'.php';
        if (is_file($path)) { require $path; return; }
    }
}, true, true);

use Greenter\Ws\Reader\XmlReader;
use Greenter\Ws\Reader\DomCdrReader;
use Greenter\Ws\Reader\XmlFilenameExtractor;
use Greenter\Ws\Resolver\XmlTypeResolver;
use Greenter\Ws\Services\BillSender;
use Greenter\Ws\Services\SummarySender;
use Greenter\Ws\Services\ExtService;
use Greenter\Ws\Services\ConsultCdrService;
use Greenter\Ws\Services\WsClientInterface;
use Greenter\Ws\Services\WsdlProvider;
use Greenter\Ws\Services\SunatEndpoints;
use Greenter\Ws\Services\SoapClient as PhpSoapClient;
use Greenter\Zip\ZipFly;
use Greenter\Api\ApiFactory;
use Greenter\Api\GreSender;
use Greenter\Api\InMemoryStore;
use Greenter\Services\Api\BasicToken;
use Greenter\Sunat\GRE\Api\AuthApiInterface;
use Greenter\Sunat\GRE\Api\CpeApiInterface;
use Greenter\Sunat\GRE\ApiException;
use Greenter\Sunat\GRE\Model\ApiToken;
use Greenter\Sunat\GRE\Model\CpeResponse;
use Greenter\Sunat\GRE\Model\StatusResponse;
use Greenter\Sunat\GRE\Model\StatusResponseError;
use Greenter\Sunat\GRE\Model\CpeError;
use Greenter\Sunat\GRE\Model\CpeErrorValidation;
use GuzzleHttp\ClientInterface;

final class LocalWsClient implements WsClientInterface
{
    public array $calls = [];
    public function __construct(private object $reply) {}
    public function call($function, $arguments)
    {
        $params = $arguments['parameters'] ?? [];
        if (isset($params['contentFile'])) {
            $zip = new ZipFly();
            $entries = $zip->decompress($params['contentFile']);
            $params['contentFile'] = array_map(static fn($entry) => [
                'filename' => $entry['filename'],
                'content' => $entry['content'],
            ], $entries);
        }
        $this->calls[] = ['method' => $function, 'params' => $params];
        if ($this->reply instanceof Throwable) throw $this->reply;
        return $this->reply;
    }
}

function cdr_data($cdr): ?array
{
    if ($cdr === null) return null;
    return [
        'id' => $cdr->getId(), 'code' => $cdr->getCode(),
        'description' => $cdr->getDescription(), 'notes' => $cdr->getNotes(),
        'reference' => $cdr->getReference(),
    ];
}

function result_data($result): array
{
    $error = $result->getError();
    $out = ['success' => $result->isSuccess(),
        'error' => $error ? ['code' => $error->getCode(), 'message' => $error->getMessage()] : null];
    if (method_exists($result, 'getTicket')) $out['ticket'] = $result->getTicket();
    if (method_exists($result, 'getCode')) $out['code'] = $result->getCode();
    if (method_exists($result, 'getMessage')) $out['message'] = $result->getMessage();
    if (method_exists($result, 'getCdrResponse')) $out['cdr'] = cdr_data($result->getCdrResponse());
    if (method_exists($result, 'getCdrZip')) {
        $zip = $result->getCdrZip();
        $out['has_cdr_zip'] = $zip !== null && $zip !== '';
    }
    return $out;
}

$cases = json_decode(file_get_contents($argv[1] ?? ''), true, 512, JSON_THROW_ON_ERROR)['cases'];
$cdrXml = null;
foreach ($cases as $case) if ($case['id'] === 'cdr-ok') $cdrXml = $case['xml'];
$out = [];
foreach ($cases as $case) {
    $warnings = [];
    $client = null;
    set_error_handler(static function ($severity, $message) use (&$warnings): bool {
        if (($severity & E_DEPRECATED) === 0) $warnings[] = $message;
        return true;
    });
    $calls = [];
    try {
        $kind = $case['kind'];
        if ($kind === 'endpoints') {
            $value = [
                'fe_beta' => SunatEndpoints::FE_BETA,
                'fe_homologacion' => SunatEndpoints::FE_HOMOLOGACION,
                'fe_produccion' => SunatEndpoints::FE_PRODUCCION,
                'fe_consulta_cdr' => SunatEndpoints::FE_CONSULTA_CDR,
                'guia_beta' => SunatEndpoints::GUIA_BETA,
                'guia_produccion' => SunatEndpoints::GUIA_PRODUCCION,
                'retencion_beta' => SunatEndpoints::RETENCION_BETA,
                'retencion_produccion' => SunatEndpoints::RETENCION_PRODUCCION,
                'wsdl_endpoint' => SunatEndpoints::WSDL_ENDPOINT,
            ];
        } elseif ($kind === 'zip') {
            $zipper = new ZipFly();
            $bytes = $zipper->compress($case['name'], $case['content']);
            $entries = $zipper->decompress($bytes);
            // nelexa/zip escribe getdate(time()) en la hora/fecha MS-DOS de la
            // cabecera local (desplazamiento 10) y del directorio central
            // (desplazamiento 12 desde su firma). Solo esos cuatro bytes se
            // ponen a cero antes del hash para que el fixture sea reproducible;
            // Rust genera el mismo ZIP con ZipTime::Fixed(DosDateTime::MIN).
            $normalized = $bytes;
            // Sin comentario de archivo: el EOCD ocupa los últimos 22 bytes y
            // guarda en +16 el desplazamiento del directorio central.
            $central = unpack('V', substr($normalized, -6, 4))[1];
            if (substr($normalized, $central, 4) !== "PK\x01\x02") {
                throw new RuntimeException('ZIP sin directorio central');
            }
            $normalized = substr_replace($normalized, "\0\0\0\0", 10, 4);
            $normalized = substr_replace($normalized, "\0\0\0\0", $central + 12, 4);
            $value = [
                'entries' => $entries,
                'length' => strlen($bytes),
                'sha256_without_dos_time' => hash('sha256', $normalized),
            ];
        } elseif ($kind === 'cdr') {
            $value = cdr_data((new DomCdrReader(new XmlReader()))->getCdrResponse($case['xml']));
        } elseif ($kind === 'name') {
            $value = (new XmlFilenameExtractor(new XmlReader()))->getFilename($case['xml']);
        } elseif ($kind === 'type') {
            $value = (new XmlTypeResolver(new XmlReader()))->getType($case['xml']);
        } elseif ($kind === 'gre-send') {
            $capture = null;
            $api = Mockery::mock(CpeApiInterface::class);
            $api->shouldReceive('enviarCpe')->andReturnUsing(static function ($filename, $document) use (&$capture) {
                $file = $document->getArchivo();
                $bytes = base64_decode($file->getArcGreZip(), true);
                $capture = [
                    'filename' => $filename,
                    'nomArchivo' => $file->getNomArchivo(),
                    'hash_matches' => hash('sha256', $bytes) === $file->getHashZip(),
                    'entries' => (new ZipFly())->decompress($bytes),
                ];
                return new CpeResponse(['num_ticket' => 'TICKET-FICTICIO']);
            });
            $value = result_data((new GreSender($api))->send($case['name'], $case['content']));
            $calls = [$capture];
        } elseif ($kind === 'gre-status') {
            $mode = $case['response'];
            $api = Mockery::mock(CpeApiInterface::class);
            if ($mode === '422') {
                $ex = new ApiException('ERROR HTTP FICTICIO', 422);
                $ex->setResponseObject(new CpeErrorValidation(['errors' => [new CpeError(['cod' => '501', 'msg' => 'Error ficticio'])]]));
                $api->shouldReceive('consultarEnvio')->andThrow($ex);
            } else {
                $data = ['cod_respuesta' => $mode, 'ind_cdr_generado' => $mode === '0' ? '1' : '0'];
                if ($mode === '99') $data['error'] = new StatusResponseError(['num_error' => '1345', 'des_error' => 'Error ficticio']);
                if ($mode === '0') $data['arc_cdr'] = base64_encode((new ZipFly())->compress('R-20000000001-01-F001-1.xml', $cdrXml));
                $api->shouldReceive('consultarEnvio')->andReturn(new StatusResponse($data));
            }
            $value = result_data((new GreSender($api))->status($case['ticket']));
            $calls = [['ticket' => $case['ticket']]];
        } elseif ($kind === 'factory') {
            $mode = $case['response'];
            $authCalls = 0;
            $auth = Mockery::mock(AuthApiInterface::class);
            $auth->shouldReceive('getToken')->andReturnUsing(static function () use (&$authCalls, $mode) {
                $authCalls++;
                return new ApiToken(['access_token' => $mode === 'empty' ? '' : 'TOKEN-FICTICIO', 'expires_in' => 3600]);
            });
            $store = new InMemoryStore();
            if ($mode === 'cache') $store->set('CLIENTE-FICTICIO', new BasicToken('TOKEN-CACHE', new DateTime('+1 hour')));
            if ($mode === 'expired') $store->set('CLIENTE-FICTICIO', new BasicToken('TOKEN-CADUCADO', new DateTime('-1 minute')));
            $factory = new ApiFactory($auth, Mockery::mock(ClientInterface::class), $store, null);
            try {
                $cpe = $factory->create('CLIENTE-FICTICIO', 'SECRETO-FICTICIO', 'USUARIO-FICTICIO', 'CLAVE-FICTICIA');
                $value = [
                    'auth_calls' => $authCalls,
                    'host' => $cpe->getConfig()->getHost(),
                    'has_token' => !empty($cpe->getConfig()->getAccessToken()),
                    'stored' => $store->get('CLIENTE-FICTICIO') !== null,
                ];
            } catch (Throwable $error) {
                $calls = [['auth_calls' => $authCalls]];
                throw $error;
            }
            $calls = [['auth_calls' => $authCalls]];
        } elseif ($kind === 'soap') {
            $base = getenv('WS_SOAP_URL');
            $parts = parse_url((string) $base);
            if (!$parts || ($parts['scheme'] ?? '') !== 'http' || ($parts['host'] ?? '') !== '127.0.0.1') {
                throw new RuntimeException('SOAP local obligatorio');
            }
            $consult = $case['operation'] === 'getStatusCdr';
            $client = new PhpSoapClient($consult ? WsdlProvider::getConsultPath() : WsdlProvider::getBillPath());
            $client->setCredentials('USUARIO-FICTICIO', 'CLAVE-FICTICIA');
            $client->setService($base.'/'.$case['id']);
            if ($case['operation'] === 'sendBill') $service = new BillSender();
            elseif ($case['operation'] === 'sendSummary') $service = new SummarySender();
            elseif ($consult) $service = new ConsultCdrService();
            else $service = new ExtService();
            $service->setClient($client);
            $result = match ($case['operation']) {
                'sendBill', 'sendSummary' => $service->send($case['name'], $case['content']),
                'getStatus' => $service->getStatus($case['ticket']),
                'getStatusCdr' => $service->getStatusCdr($case['ruc'], $case['tipo'], $case['serie'], $case['numero']),
            };
            $value = result_data($result);
        } else {
            $cdrZip = (new ZipFly())->compress('R-20000000001-01-F001-1.xml', $cdrXml);
            $mode = $case['response'];
            $reply = match ($kind) {
                'bill' => $mode === 'fault' ? new SoapFault('0156', 'ERROR TEST') : (object)['applicationResponse' => $mode === 'cdr' ? $cdrZip : null],
                'summary' => $mode === 'fault' ? new SoapFault('2001', 'ERROR TEST') : (object)['ticket' => 'TICKET-FICTICIO'],
                'ext' => match ($mode) {
                    'pending' => (object)['status' => (object)['statusCode' => '98']],
                    'missing' => (object)[],
                    'empty' => (object)['status' => (object)['statusCode' => '0', 'content' => '']],
                    default => (object)['status' => (object)['statusCode' => '0', 'content' => $cdrZip]],
                },
                'consult' => (object)[($mode === 'cdr' ? 'statusCdr' : 'status') =>
                    (object)['statusCode' => '0', 'statusMessage' => 'ACEPTADA',
                        'content' => $mode === 'cdr' ? $cdrZip : null]],
            };
            $client = new LocalWsClient($reply);
            if ($kind === 'bill') $service = new BillSender();
            elseif ($kind === 'summary') $service = new SummarySender();
            elseif ($kind === 'ext') $service = new ExtService();
            else $service = new ConsultCdrService();
            $service->setClient($client);
            $result = match ($kind) {
                'bill', 'summary' => $service->send($case['name'], $case['content']),
                'ext' => $service->getStatus($case['ticket']),
                'consult' => $mode === 'cdr'
                    ? $service->getStatusCdr($case['ruc'], $case['tipo'], $case['serie'], $case['numero'])
                    : $service->getStatus($case['ruc'], $case['tipo'], $case['serie'], $case['numero']),
            };
            $value = result_data($result);
            $calls = $client->calls;
        }
        $out[$case['id']] = ['result' => $value, 'calls' => $calls, 'warnings' => $warnings];
    } catch (Throwable $error) {
        if ($client instanceof LocalWsClient) $calls = $client->calls;
        $out[$case['id']] = ['error' => ['class' => get_class($error), 'message' => $error->getMessage()],
            'calls' => $calls, 'warnings' => $warnings];
    } finally {
        if (class_exists(Mockery::class, false)) Mockery::close();
        restore_error_handler();
    }
}
echo json_encode(['reference_version' => PHP_VERSION, 'results' => $out], JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PRETTY_PRINT), "\n";
