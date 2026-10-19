<?php
declare(strict_types=1);

// Oráculo explícito del lite fijado; nunca crea clientes con destinos remotos.
ini_set('display_errors', '0');
error_reporting(E_ALL);
date_default_timezone_set('UTC');
ini_set('precision', '14');

$root = dirname(__DIR__, 3);
$ref = $root.'/target/reference/v5.3.0';
if (trim((string) shell_exec('git -C '.escapeshellarg($ref).' rev-parse HEAD')) !== 'f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f') {
    throw new RuntimeException('Referencia PHP incorrecta');
}
require $root.'/oracle/php/core/lib.php';
require $root.'/target/reference/v5.3.0-suite/vendor/autoload.php';
spl_autoload_register(static function (string $class) use ($ref): void {
    if (!str_starts_with($class, 'Greenter\\')) return;
    $relative = str_replace('\\', '/', substr($class, 9)).'.php';
    foreach (['lite/src/Greenter', 'xml/src', 'ws/src', 'core/src/Core'] as $part) {
        $file = "$ref/packages/$part/$relative";
        if (is_file($file)) { require $file; return; }
    }
}, true, true);
// Identidad CPEForge en la firma y en la URI de las plantillas.
require $root.'/oracle/php/identidad.php';

use Greenter\Api as LiteApi;
use Greenter\See;
use Greenter\Api\ApiFactory;
use Greenter\Builder\BuilderInterface;
use Greenter\Factory\FeFactory;
use Greenter\Factory\XmlBuilderResolver;
use Greenter\Model\DocumentInterface;
use Greenter\Model\Response\BaseResult;
use Greenter\Model\Response\SummaryResult;
use Greenter\Services\SenderInterface;
use Greenter\Validator\ErrorCodeProviderInterface;
use Greenter\Sunat\GRE\Api\CpeApiInterface;
use Greenter\Sunat\GRE\ApiException;
use Greenter\Sunat\GRE\Model\CpeDocument;
use Greenter\Sunat\GRE\Model\CpeResponse;
use Greenter\Sunat\GRE\Model\CpeError;
use Greenter\Sunat\GRE\Model\StatusResponse;
use Greenter\Ws\Services\WsClientInterface;
use Greenter\Zip\ZipFly;
use Greenter\XMLSecLibs\Sunat\SignedXml;

final class FixedBuilder implements BuilderInterface {
    public bool $fail = false;
    public function build(DocumentInterface $document): ?string {
        if ($this->fail) throw new RuntimeException('builder failed');
        return '<Invoice/>';
    }
}
final class FixedSigner extends SignedXml {
    public bool $fail = false;
    public function signXml($content) {
        if ($this->fail) throw new RuntimeException('signer failed');
        return 'signed:'.$content;
    }
}
/**
 * SHA-256 del ZIP con la hora/fecha MS-DOS a cero. nelexa/zip escribe
 * getdate(time()) en la cabecera local (+10) y en el directorio central
 * (+12); solo esos bytes dependen de la ejecución. Rust aplica la misma
 * normalización en crates/cpeforge/tests/live_parity.rs.
 */
function zip_hash_without_dos_time(string $zip): string {
    $central = unpack('V', substr($zip, -6, 4))[1];
    if (substr($zip, $central, 4) !== "PK\x01\x02") {
        throw new RuntimeException('ZIP sin directorio central');
    }
    $zip = substr_replace($zip, "\0\0\0\0", 10, 4);
    return hash('sha256', substr_replace($zip, "\0\0\0\0", $central + 12, 4));
}
final class FixedSender implements SenderInterface {
    public array $calls = [];
    public bool $fail = false;
    public function send(?string $filename, ?string $content): ?BaseResult {
        $this->calls[] = ['name' => $filename, 'xml' => $content];
        if ($this->fail) throw new RuntimeException('sender failed');
        return (new SummaryResult())->setTicket('T-123')->setSuccess(true);
    }
}
final class LocalWsClient implements WsClientInterface {
    public array $calls = [];
    public bool $fault = false;
    public function call($function, $arguments) {
        $params = $arguments['parameters'] ?? [];
        if (isset($params['contentFile'])) {
            $params['zip_hash_without_dos_time'] = zip_hash_without_dos_time($params['contentFile']);
            $params['zip_bytes'] = strlen($params['contentFile']);
            $entries = (new ZipFly())->decompress($params['contentFile']);
            $params['contentFile'] = array_map(static fn($e) => ['filename' => $e['filename'], 'content' => $e['content']], $entries);
        }
        $this->calls[] = ['operation' => $function, 'params' => $params];
        if ($this->fault) throw new SoapFault('soap:Client.0100', 'Original');
        if ($function === 'sendSummary') return (object) ['ticket' => 'T-123'];
        if ($function === 'getStatus') return (object) ['status' => (object) ['statusCode' => '98']];
        throw new RuntimeException('operación SOAP no simulada');
    }
}
final class LocalCodes implements ErrorCodeProviderInterface {
    public function getAll(): ?array { return ['0100' => 'Mensaje ficticio']; }
    public function getValue(?string $code): ?string { return $code === '0100' ? 'Mensaje ficticio' : null; }
}
final class LocalCpe implements CpeApiInterface {
    public array $calls = [];
    public bool $serverError = false;
    public function enviarCpe(string $filename, CpeDocument $document): CpeResponse {
        if ($this->serverError) {
            $error = new ApiException('fallo ficticio', 500);
            $error->setResponseObject((new CpeError())->setCod('5000')->setMsg('Fallo ficticio'));
            throw $error;
        }
        $archive = $document->getArchivo();
        $zip = base64_decode($archive->getArcGreZip(), true);
        $entries = (new ZipFly())->decompress($zip);
        $this->calls[] = ['operation' => 'enviarCpe', 'name' => $filename,
            'zip_name' => $archive->getNomArchivo(),
            'zip_hash_without_dos_time' => zip_hash_without_dos_time($zip),
            'zip_hash_valid' => hash('sha256', $zip) === $archive->getHashZip(),
            'entries' => array_map(static fn($e) => ['filename' => $e['filename'], 'content' => $e['content']], $entries)];
        return (new CpeResponse())->setNumTicket('T-123');
    }
    public function consultarEnvio(string $ticket): StatusResponse {
        $this->calls[] = ['operation' => 'consultarEnvio', 'ticket' => $ticket];
        return (new StatusResponse())->setCodRespuesta('98');
    }
}
final class LocalFactory extends ApiFactory {
    public function __construct(public LocalCpe $cpe) {}
    public function create(?string $clientId, ?string $secret, ?string $user, ?string $password): CpeApiInterface {
        if ([$clientId, $secret, $user, $password] !== ['client.test', 'secret.test', '20000000001TEST', 'password.test']) {
            throw new RuntimeException('credenciales ficticias no coinciden');
        }
        return $this->cpe;
    }
}

function lite_error_info(Throwable $e): array { return ['class' => get_class($e), 'message' => $e->getMessage()]; }
function lite_result_info(BaseResult $r): array {
    $error = $r->getError();
    return ['success' => $r->isSuccess(), 'ticket' => method_exists($r, 'getTicket') ? $r->getTicket() : null,
        'error' => $error ? ['code' => $error->getCode(), 'message' => $error->getMessage()] : null];
}

$keys = $argv[1] ?? '';
$pem = (string) file_get_contents($keys.'/a.pem');
$invoiceCases = json_decode((string) file_get_contents($root.'/tests/reference/xml/cases/invoice.json'), true, 512, JSON_THROW_ON_ERROR)['cases'];
$summaryCases = json_decode((string) file_get_contents($root.'/tests/reference/xml/cases/summary.json'), true, 512, JSON_THROW_ON_ERROR)['cases'];
$find = static fn(array $cases, string $id): array => array_values(array_filter($cases, static fn($c) => $c['id'] === $id))[0];
$invoice = hydrate($find($invoiceCases, 'inv21-full-cpeforge')['input']);
$summary = hydrate($find($summaryCases, 'summary-full-cpeforge')['input']);
$out = [];
$buildResolver = new XmlBuilderResolver(['autoescape' => false]);
$out['invoice_pre_sign'] = $buildResolver->find(get_class($invoice))->build($invoice);
$out['summary_pre_sign'] = $buildResolver->find(get_class($summary))->build($summary);

$factory = new FeFactory(); $builder = new FixedBuilder(); $signer = new FixedSigner(); $sender = new FixedSender();
$factory->setBuilder($builder)->setSigner($signer)->setSender($sender);
$out['factory_initial'] = $factory->getLastXml();
$out['factory_send'] = lite_result_info($factory->send($invoice));
$out['factory_after_send'] = $factory->getLastXml();
$out['factory_raw'] = lite_result_info($factory->sendXml('raw', '<already-signed/>'));
$out['factory_after_raw'] = $factory->getLastXml();
$builder->fail = true;
try { $factory->send($invoice); } catch (Throwable $e) { $out['factory_builder_error'] = lite_error_info($e); }
$out['factory_after_builder_error'] = $factory->getLastXml(); $builder->fail = false;
$signer->fail = true;
try { $factory->send($invoice); } catch (Throwable $e) { $out['factory_signer_error'] = lite_error_info($e); }
$out['factory_after_signer_error'] = $factory->getLastXml(); $signer->fail = false;
$sender->fail = true;
try { $factory->send($invoice); } catch (Throwable $e) { $out['factory_sender_error'] = lite_error_info($e); }
$out['factory_after_sender_error'] = $factory->getLastXml();
$out['factory_calls'] = $sender->calls;

$ws = new LocalWsClient(); $see = new See();
(new ReflectionProperty(See::class, 'wsClient'))->setValue($see, $ws);
$see->setCertificate($pem);
$out['see_xml_before'] = $see->getFactory()->getLastXml();
$out['see_signed_summary'] = $see->getXmlSigned($summary);
$out['see_xml_after_signed'] = $see->getFactory()->getLastXml();
$out['see_send_xml'] = lite_result_info($see->sendXml(get_class($summary), $summary->getName(), '<caller-xml/>'));
$out['see_xml_after_raw'] = $see->getFactory()->getLastXml();
$out['see_send_xml_file'] = lite_result_info($see->sendXmlFile($out['see_signed_summary']));
$out['see_xml_after_file'] = $see->getFactory()->getLastXml();
$out['see_send'] = lite_result_info($see->send($summary));
$out['see_xml_after_send'] = $see->getFactory()->getLastXml();
$out['see_status'] = lite_result_info($see->getStatus('T-123'));
$out['see_calls'] = $ws->calls;
$faultWs = new LocalWsClient(); $faultWs->fault = true;
$faultSee = new See();
(new ReflectionProperty(See::class, 'wsClient'))->setValue($faultSee, $faultWs);
$faultSee->setCodeProvider(new LocalCodes());
$out['see_fault'] = lite_result_info($faultSee->sendXml(get_class($invoice), 'fake', '<raw/>'));
try { $see->sendXml('Example\\Unknown', 'x', '<raw/>'); } catch (Throwable $e) { $out['see_unknown_error'] = lite_error_info($e); }
try { $see->sendXmlFile('<broken'); } catch (Throwable $e) { $out['see_invalid_xml_error'] = lite_error_info($e); }

$cpe = new LocalCpe(); $api = new LiteApi(null, new LocalFactory($cpe), new SignedXml());
$api->setCertificate($pem)->setApiCredentials('client.test', 'secret.test')->setClaveSOL('20000000001', 'TEST', 'password.test');
$out['api_xml_before'] = $api->getLastXml();
$out['api_send_xml'] = lite_result_info($api->sendXml('raw', '<caller-xml/>'));
$out['api_xml_after_raw'] = $api->getLastXml();
$out['api_send'] = lite_result_info($api->send($invoice));
$out['api_xml_after_send'] = $api->getLastXml();
$out['api_status'] = lite_result_info($api->getStatus('T-123'));
$cpe->serverError = true;
$out['api_server_error'] = lite_result_info($api->sendXml('error', '<caller-xml/>'));
$out['api_xml_after_server_error'] = $api->getLastXml();
$out['api_send_error'] = lite_result_info($api->send($invoice));
$out['api_xml_after_send_error'] = $api->getLastXml();
$cpe->serverError = false;
$out['api_calls'] = $cpe->calls;
$api->setCertificate('PEM NO VALIDO');
try { $api->send($invoice); } catch (Throwable $e) { $out['api_sign_error'] = lite_error_info($e); }
$out['api_xml_after_sign_error'] = $api->getLastXml();

$invoice->getClient()->setRznSocial('CLIENTE & SINTETICO');
$htmlSee = new See();
$htmlSee->setBuilderOptions(['strict_variables' => true]);
$htmlSee->setBuilderOptions(['autoescape' => 'html']);
$htmlSee->setCachePath(null);
$htmlSee->setCertificate($pem);
$out['html_invoice_pre_sign'] = (new XmlBuilderResolver(['autoescape' => 'html', 'strict_variables' => true]))->find(get_class($invoice))->build($invoice);
$out['html_invoice_signed'] = $htmlSee->getXmlSigned($invoice);

echo json_encode($out, JSON_THROW_ON_ERROR | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES), "\n";
