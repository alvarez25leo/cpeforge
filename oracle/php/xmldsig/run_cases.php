<?php

declare(strict_types=1);

/*
 * Oráculo PHP de greenter/xmldsig v5.1.0.
 *
 * Cada caso nombra un escenario (una secuencia de llamadas a la API pública
 * PHP) y sus parámetros. Las claves se referencian por nombre ("a", "b",
 * "expired", ...) y se leen del directorio EFÍMERO pasado como argumento,
 * creado por scripts/xmldsig_keys.sh; nunca se escriben en la salida salvo
 * dentro de resultados que el arnés redacta antes de guardar fixtures.
 *
 * Uso: php oracle/php/xmldsig/run_cases.php <casos.json> <dir_claves> [dir_entradas]
 *
 * dir_entradas permite verificar XML firmados por Rust: el caso indica
 * "input_file" y el oráculo lo lee de ese directorio.
 */

$root = dirname(__DIR__, 3);
$reference = json_decode((string) file_get_contents($root . '/oracle/upstream/referencia-xmldsig.json'), true);
$refDir = getenv('XMLDSIG_PHP_REFERENCE') ?: $root . '/target/reference/xmldsig-v5.1.0';
if (!is_file($refDir . '/src/XMLSecurityDSig.php')) {
    fwrite(STDERR, "Falta la referencia en $refDir; ejecutar scripts/xmldsig_parity.sh\n");
    exit(2);
}
spl_autoload_register(static function (string $class) use ($refDir): void {
    $prefix = 'Greenter\\XMLSecLibs\\';
    if (strncmp($class, $prefix, strlen($prefix)) === 0) {
        $file = $refDir . '/src/' . str_replace('\\', '/', substr($class, strlen($prefix))) . '.php';
        if (is_file($file)) {
            require $file;
        }
    }
});

use Greenter\XMLSecLibs\Certificate\X509Certificate;
use Greenter\XMLSecLibs\Certificate\X509ContentType;
use Greenter\XMLSecLibs\Sunat\SignedXml;
use Greenter\XMLSecLibs\Util\XPath;
use Greenter\XMLSecLibs\XMLSecEnc;
use Greenter\XMLSecLibs\XMLSecurityDSig;
use Greenter\XMLSecLibs\XMLSecurityKey;

ini_set('precision', '14');
date_default_timezone_set('UTC');

/** @var list<string> */
$warnings = [];
set_error_handler(static function (int $errno, string $message) use (&$warnings, $refDir): bool {
    $warnings[] = str_replace($refDir, '<ref>', $message);
    return true;
});

$casesFile = $argv[1] ?? '';
$keysDir = $argv[2] ?? '';
$inputsDir = $argv[3] ?? null;
$cases = json_decode((string) file_get_contents($casesFile), true, 512, JSON_THROW_ON_ERROR);

/**
 * Implementación de iconv que usa libxml2, detectada por su comportamiento:
 * el iconv de glibc rechaza el octeto 0x81 en windows-1252 (sin asignar) y el
 * libiconv de macOS lo lee como control C1. Algunos casos de `encoding`
 * dependen de esto (tests/reference/xmldsig/differences.json, only_when).
 */
function iconv_profile(): string
{
    $previous = libxml_use_internal_errors(true);
    $loaded = (new DOMDocument())->loadXML("<?xml version=\"1.0\" encoding=\"windows-1252\"?><r>\x81</r>");
    libxml_clear_errors();
    libxml_use_internal_errors($previous);
    return $loaded ? 'libiconv' : 'glibc';
}

/**
 * Perfiles de libxml2 que cambian resultados de PHP según su versión (2.9 en
 * Ubuntu frente a 2.15 en macOS), detectados por comportamiento. Los casos
 * afectados llevan `only_when` en tests/reference/xmldsig/differences.json.
 *
 * - libxml_cdata: la 2.9 une dos secciones CDATA contiguas en un nodo
 *   (serialize-cdata).
 * - libxml_cr: la 2.9 serializa el retorno de carro como `&#xD;` y la
 *   reciente como `&#13;` (serialize-text-escapes).
 */
function libxml_cdata(): string
{
    $doc = new DOMDocument();
    $doc->loadXML('<a><![CDATA[a]]><![CDATA[b]]></a>');

    return $doc->documentElement->childNodes->length === 1 ? 'unidos' : 'separados';
}

function libxml_cr(): string
{
    $doc = new DOMDocument();
    $doc->loadXML('<r>a&#13;b</r>');

    // Documento completo, como el caso: en la 2.9 la serialización de un nodo
    // suelto (xmlNodeDump) y la del documento escapan el CR de forma distinta.
    return str_contains((string) $doc->saveXML(), '&#xD;') ? 'hex' : 'decimal';
}

/**
 * libxml_xpath: la 2.9 no evalúa el filtro XPath de los casos
 * `c14n-xpath-text-functions-*` y PHP lanza «XPath query did not return a
 * nodeset». Una sonda aparte no reprodujo el fallo, así que el perfil se lee
 * del resultado PHP de ese caso (solo en la familia que lo contiene).
 *
 * @param array<string, mixed> $results
 */
function libxml_xpath(array $results): ?string
{
    $case = $results['c14n-xpath-text-functions-document'] ?? null;
    if ($case === null) {
        return null;
    }

    return str_contains((string) ($case['error']['message'] ?? ''), 'did not return a nodeset') ? 'limitado' : 'completo';
}

function key_file(string $name, string $suffix = 'pem'): string
{
    global $keysDir;
    $path = "$keysDir/$name.$suffix";
    if (!is_file($path)) {
        throw new LogicException("material de prueba inexistente: $name.$suffix");
    }
    return (string) file_get_contents($path);
}

/** Texto: cadena UTF-8 tal cual o {"b64": ...} si no es UTF-8 válido. */
function out_bytes(?string $bytes): mixed
{
    if ($bytes === null) {
        return null;
    }
    // preg_match con /u valida UTF-8 sin mbstring: el oráculo corre con
    // `php -n` y en Ubuntu mbstring es una extensión compartida.
    return preg_match('//u', $bytes) === 1 ? $bytes : ['b64' => base64_encode($bytes)];
}

function in_bytes(mixed $value): string
{
    if (is_array($value) && isset($value['b64'])) {
        return (string) base64_decode($value['b64'], true);
    }
    if (is_array($value) && isset($value['key'])) {
        return key_file($value['key'], $value['suffix'] ?? 'pem');
    }
    if (is_array($value) && isset($value['file'])) {
        global $root;
        return (string) file_get_contents($root . '/' . $value['file']);
    }
    if (is_array($value) && isset($value['key_info_cert'])) {
        // KeyInfo con el certificado (cuerpo Base64 del PEM, con sus saltos de línea).
        $pem = key_file($value['key_info_cert'], 'crt');
        $body = trim(preg_replace('/-----[A-Z ]+-----/', '', $pem));
        return '<root xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><t><ds:KeyInfo><ds:X509Data><ds:X509Certificate>'
            . "\n" . $body . "\n" . '</ds:X509Certificate></ds:X509Data></ds:KeyInfo></t></root>';
    }
    if (is_array($value) && isset($value['repeat'])) {
        $r = $value['repeat'];
        return $r['prefix'] . str_repeat($r['unit'], $r['count']) . $r['suffix'];
    }
    if (is_array($value) && isset($value['input_file'])) {
        global $inputsDir;
        return (string) file_get_contents($inputsDir . '/' . $value['input_file']);
    }
    return (string) $value;
}

function load_doc(string $xml, array $case): DOMDocument
{
    $doc = new DOMDocument();
    if (array_key_exists('preserve_whitespace', $case)) {
        $doc->preserveWhiteSpace = (bool) $case['preserve_whitespace'];
    }
    $ok = $doc->loadXML($xml);
    if ($ok === false) {
        throw new RuntimeException('loadXML devolvió false');
    }
    return $doc;
}

/** Elemento destino: "document", "root", "id:VALOR" (atributo Id) o "tag:ns|local[n]". */
function target(DOMDocument $doc, string $spec): DOMNode
{
    if ($spec === 'document') {
        return $doc;
    }
    if ($spec === 'root') {
        return $doc->documentElement;
    }
    if (str_starts_with($spec, 'id:')) {
        $xp = new DOMXPath($doc);
        $node = $xp->query('//*[@Id="' . substr($spec, 3) . '"]')->item(0);
        if (!$node) {
            throw new LogicException("destino inexistente $spec");
        }
        return $node;
    }
    if (preg_match('/^tag:([^|]*)\|([^\[]+)(?:\[(\d+)\])?$/', $spec, $m)) {
        $node = $doc->getElementsByTagNameNS($m[1], $m[2])->item((int) ($m[3] ?? 0));
        if (!$node) {
            throw new LogicException("destino inexistente $spec");
        }
        return $node;
    }
    throw new LogicException("destino desconocido $spec");
}

/** Referencia ds:Reference con transforms, en un documento propio. */
function reference_node(array $transforms): DOMElement
{
    $ns = XMLSecurityDSig::XMLDSIGNS;
    $doc = new DOMDocument();
    $ref = $doc->createElementNS($ns, 'ds:Reference');
    $doc->appendChild($ref);
    $trs = $doc->createElementNS($ns, 'ds:Transforms');
    $ref->appendChild($trs);
    foreach ($transforms as $t) {
        $tr = $doc->createElementNS($ns, 'ds:Transform');
        $trs->appendChild($tr);
        $alg = is_array($t) ? $t['algorithm'] : $t;
        $tr->setAttribute('Algorithm', $alg);
        if (is_array($t) && isset($t['prefix_list'])) {
            $inc = $doc->createElementNS('http://www.w3.org/2001/10/xml-exc-c14n#', 'ec:InclusiveNamespaces');
            $inc->setAttribute('PrefixList', $t['prefix_list']);
            $tr->appendChild($inc);
        }
        if (is_array($t) && isset($t['xpath'])) {
            $x = $doc->createElementNS($ns, 'ds:XPath', $t['xpath']);
            $tr->appendChild($x);
            foreach ($t['xpath_ns'] ?? [] as $prefix => $uri) {
                $x->setAttributeNS('http://www.w3.org/2000/xmlns/', $prefix === '' ? 'xmlns' : "xmlns:$prefix", $uri);
            }
        }
    }
    return $ref;
}

/** Aplica mutaciones de texto a un XML serializado. */
function mutate(string $xml, array $mutations): string
{
    foreach ($mutations as $m) {
        switch ($m['op']) {
            case 'replace':
                $pos = strpos($xml, $m['from']);
                if ($pos === false) {
                    throw new LogicException('mutación sin coincidencia: ' . $m['from']);
                }
                $xml = substr_replace($xml, $m['to'], $pos, strlen($m['from']));
                break;
            case 'element_text':
                // Reemplaza el texto del n-ésimo elemento <prefijo:local>…</…>.
                $re = '#(<' . preg_quote($m['tag'], '#') . '(?:\s[^>]*)?>)([^<]*)(</' . preg_quote($m['tag'], '#') . '>)#';
                $count = 0;
                $xml = preg_replace_callback($re, static function ($mm) use (&$count, $m) {
                    $hit = $count++ === ($m['index'] ?? 0);
                    return $hit ? $mm[1] . $m['to'] . $mm[3] : $mm[0];
                }, $xml);
                break;
            case 'flip_base64':
                // Cambia un carácter del n-ésimo valor Base64 de la etiqueta.
                $re = '#(<' . preg_quote($m['tag'], '#') . '(?:\s[^>]*)?>)([^<]*)(</' . preg_quote($m['tag'], '#') . '>)#';
                $xml = preg_replace_callback($re, static function ($mm) {
                    $v = $mm[2];
                    $c = $v[4] === 'A' ? 'B' : 'A';
                    return $mm[1] . substr($v, 0, 4) . $c . substr($v, 5) . $mm[3];
                }, $xml, 1);
                break;
            default:
                throw new LogicException('mutación desconocida ' . $m['op']);
        }
    }
    return $xml;
}

function dsig_from_case(array $case): XMLSecurityDSig
{
    return array_key_exists('prefix', $case) ? new XMLSecurityDSig($case['prefix'] ?? '') : new XMLSecurityDSig();
}

function new_key(array $spec): XMLSecurityKey
{
    $key = isset($spec['type']) ? new XMLSecurityKey($spec['alg'], ['type' => $spec['type']]) : new XMLSecurityKey($spec['alg']);
    if (isset($spec['passphrase'])) {
        $key->passphrase = $spec['passphrase'];
    }
    if (isset($spec['name'])) {
        $key->name = $spec['name'];
    }
    if (array_key_exists('load', $spec)) {
        $key->loadKey(in_bytes($spec['load']), false, (bool) ($spec['is_cert'] ?? false));
    }
    return $key;
}

/** Firma genérica con XMLSecurityDSig; devuelve el documento firmado. */
function dsig_sign(array $c): DOMDocument
{
    $doc = load_doc(in_bytes($c['xml']), $c);
    $dsig = dsig_from_case($c);
    if (isset($c['canonical'])) {
        $dsig->setCanonicalMethod($c['canonical']);
    }
    foreach ($c['references'] ?? [['target' => 'document']] as $r) {
        $node = target($doc, $r['target']);
        $dsig->addReference($node, $r['digest'] ?? XMLSecurityDSig::SHA1, $r['transforms'] ?? null, $r['options'] ?? null);
    }
    if (isset($c['object'])) {
        $dsig->addObject($c['object'], $c['object_mime'] ?? null, $c['object_encoding'] ?? null);
    }
    $key = new_key($c['key']);
    $dsig->sign($key, isset($c['append_to']) ? target($doc, $c['append_to']) : null);
    if (isset($c['cert'])) {
        $dsig->add509Cert(in_bytes($c['cert']), true, false, $c['cert_options'] ?? null);
    }
    if (isset($c['append_signature'])) {
        $dsig->appendSignature(target($doc, $c['append_signature']), (bool) ($c['insert_before'] ?? false));
    }
    return $doc;
}

function run_scenario(array $c): mixed
{
    switch ($c['scenario']) {
        case 'serialize':
            $out = load_doc(in_bytes($c['xml']), $c)->saveXML();
            // Entradas enormes: solo longitud y SHA-256 para no inflar el fixture.
            // Con una codificación declarada distinta de UTF-8 la salida
            // puede no ser UTF-8: viaja como {"b64": …}.
            return ($c['summary'] ?? false) ? ['length' => strlen($out), 'sha256' => hash('sha256', $out)] : out_bytes($out);
        case 'serialize_node':
            $doc = load_doc(in_bytes($c['xml']), $c);
            return $doc->saveXML(target($doc, $c['target']));
        case 'c14n':
            $doc = load_doc(in_bytes($c['xml']), $c);
            $dsig = new XMLSecurityDSig();
            $out = $dsig->processTransforms(reference_node($c['transforms']), target($doc, $c['target']), $c['include_comments'] ?? true);
            // DOMNode::C14N devuelve false si la consulta XPath falla.
            if ($out !== false && ($c['summary'] ?? false)) {
                return ['length' => strlen($out), 'sha256' => hash('sha256', $out)];
            }
            return $out === false ? ['c14n_failed' => true] : out_bytes($out);
        case 'digest':
            $dsig = new XMLSecurityDSig();
            return out_bytes($dsig->calculateDigest($c['alg'], in_bytes($c['data']), $c['encode'] ?? true));
        case 'xpath_filter':
            return $c['fn'] === 'value'
                ? (array_key_exists('quotes', $c) ? XPath::filterAttrValue($c['value'], $c['quotes']) : XPath::filterAttrValue($c['value']))
                : (array_key_exists('allow', $c) ? XPath::filterAttrName($c['value'], $c['allow']) : XPath::filterAttrName($c['value']));
        case 'guid':
            $g = array_key_exists('prefix', $c) ? XMLSecurityDSig::generateGUID($c['prefix']) : XMLSecurityDSig::generateGUID();
            $p = preg_quote($c['prefix'] ?? 'pfx', '/');
            return ['matches' => (bool) preg_match('/^' . $p . '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/', $g), 'length' => strlen($g)];
        case 'asn_segment':
            return out_bytes(XMLSecurityKey::makeAsnSegment($c['type'], in_bytes($c['data'])));
        case 'convert_rsa':
            return XMLSecurityKey::convertRSA(in_bytes($c['modulus']), in_bytes($c['exponent']));
        case 'x509_certs':
            return XMLSecurityDSig::staticGet509XCerts(in_bytes($c['text']), $c['pem'] ?? true);
        case 'x509_cert':
            return XMLSecurityDSig::get509XCert(in_bytes($c['text']), $c['pem'] ?? true);
        case 'thumbprint':
            return XMLSecurityKey::getRawThumbprint(in_bytes($c['text']));
        case 'key_info':
            $k = new_key($c['key']);
            return ['algorithm' => $k->getAlgorithm(), 'algorith' => $k->getAlgorith(), 'type' => $k->type, 'keysize' => $k->getSymmetricKeySize()];
        case 'session_key':
            $k = new_key($c['key']);
            $key = $k->generateSessionKey();
            $parity = true;
            for ($i = 0; $i < strlen($key); $i++) {
                $bits = 0;
                for ($j = 0; $j < 8; $j++) {
                    $bits += (ord($key[$i]) >> $j) & 1;
                }
                $parity = $parity && ($bits % 2 === 1);
            }
            return ['length' => strlen($key), 'odd_parity' => $parity, 'stored' => $k->key === $key];
        case 'symmetric_decrypt':
            $k = new_key($c['key']);
            return out_bytes($k->decryptData(in_bytes($c['data'])));
        case 'symmetric_roundtrip':
            $k = new_key($c['key']);
            $ct = $k->encryptData(in_bytes($c['data']));
            $iv = $k->iv;
            return ['ciphertext_length' => strlen($ct), 'iv_prefix' => substr($ct, 0, strlen($iv)) === $iv, 'plaintext' => out_bytes($k->decryptData($ct))];
        case 'sign_data':
            $k = new_key($c['key']);
            $sig = $k->signData(in_bytes($c['data']));
            $v = new_key($c['verify_key'] ?? $c['key']);
            return ['signature' => out_bytes($sig), 'verify' => $v->verifySignature(in_bytes($c['verify_data'] ?? $c['data']), $sig)];
        case 'rsa_roundtrip':
            $enc = new_key($c['encrypt_key']);
            $ct = $enc->encryptData(in_bytes($c['data']));
            $dec = new_key($c['decrypt_key']);
            $pt = $dec->decryptData($ct);
            // Con PKCS#1 v1.5 y clave equivocada, OpenSSL ≥ 3.2 devuelve un
            // texto sintético (rechazo implícito) que depende del cifrado
            // aleatorio: se informa solo si coincide con el original.
            $matches = $pt === in_bytes($c['data']);
            return ['ciphertext_length' => strlen($ct), 'plaintext_matches' => $matches, 'plaintext' => $matches ? out_bytes($pt) : null];
        case 'key_x509':
            $k = new_key($c['key']);
            return ['certificate' => $k->getX509Certificate(), 'thumbprint' => $k->getX509Thumbprint()];
        case 'dsig_sign':
            return dsig_sign($c)->saveXML();
        case 'dsig_verify':
            $xml = isset($c['input']) ? in_bytes($c['input']) : dsig_sign($c['sign'])->saveXML();
            $xml = mutate($xml, $c['mutations'] ?? []);
            $doc = load_doc($xml, []);
            $dsig = new XMLSecurityDSig();
            if (isset($c['id_keys'])) {
                $dsig->idKeys = $c['id_keys'];
            }
            $sig = $dsig->locateSignature($doc, $c['position'] ?? 0);
            $r = ['located' => $sig !== null];
            if (!$sig) {
                return $r;
            }
            $si = $dsig->canonicalizeSignedInfo();
            $r['signed_info'] = out_bytes($si);
            $r['ref_ids'] = $dsig->getRefIDs();
            $key = $dsig->locateKey();
            $r['key_algorithm'] = $key ? $key->getAlgorithm() : null;
            if ($key) {
                if (isset($c['verify_with'])) {
                    $key->loadKey(in_bytes($c['verify_with']), false, (bool) ($c['verify_is_cert'] ?? true));
                } else {
                    XMLSecEnc::staticLocateKeyInfo($key, $sig);
                }
                try {
                    $r['verify'] = $dsig->verify($key);
                } catch (Throwable $e) {
                    $r['verify_error'] = error_info($e);
                }
            }
            try {
                $r['references'] = $dsig->validateReference();
                $nodes = [];
                foreach ($dsig->getValidatedNodes() ?? [] as $id => $node) {
                    $nodes[] = ['key' => $id, 'node' => $node instanceof DOMDocument ? '#document' : $node->nodeName, 'text' => substr($node->textContent, 0, 40)];
                }
                $r['validated_nodes'] = $nodes;
            } catch (Throwable $e) {
                $r['references_error'] = error_info($e);
            }
            return $r;
        case 'sunat_sign':
            $s = new SignedXml();
            if (isset($c['signature_alg'])) {
                $s->setSignatureAlgorithm($c['signature_alg']);
            }
            if (isset($c['digest_alg'])) {
                $s->setDigestAlgorithm($c['digest_alg']);
            }
            if (isset($c['cert'])) {
                $s->setCertificate(in_bytes($c['cert']));
            }
            if (isset($c['cert_file'])) {
                $s->setCertificateFromFile($c['cert_file']);
            }
            return out_bytes(isset($c['sign_file']) ? $s->signFromFile($c['sign_file']) : $s->signXml(in_bytes($c['xml'])));
        case 'sunat_verify':
            $xml = in_bytes(isset($c['input']) ? $c['input'] : run_scenario(['scenario' => 'sunat_sign'] + $c['sign']));
            $xml = mutate($xml, $c['mutations'] ?? []);
            $v = new SignedXml();
            if (isset($c['configured_cert'])) {
                $v->setCertificate(in_bytes($c['configured_cert']));
            }
            if (isset($c['signature_alg'])) {
                $v->setSignatureAlgorithm($c['signature_alg']);
            }
            if ($c['dom'] ?? false) {
                $result = $v->verify(load_doc($xml, []));
            } else {
                $result = $v->verifyXml($xml);
            }
            return ['result' => $result, 'public_key_is_configured' => isset($c['configured_cert']) && $v->getPublicKey() === in_bytes($c['configured_cert'])];
        case 'enc_encrypt':
            $doc = load_doc(in_bytes($c['xml']), $c);
            $sym = new_key($c['session']);
            if ($c['generate'] ?? true) {
                $sym->generateSessionKey();
            }
            $enc = new XMLSecEnc();
            $enc->setNode(target($doc, $c['target']));
            $enc->type = $c['type'];
            if (isset($c['key_transport'])) {
                $enc->encryptKey(new_key($c['key_transport']), $sym, $c['append_key'] ?? true);
            }
            $node = $enc->encryptNode($sym, $c['replace'] ?? true);
            $xml = $node instanceof DOMDocument ? $node->saveXML() : $doc->saveXML();
            $encXml = $node instanceof DOMDocument ? $node->saveXML() : $node->ownerDocument->saveXML($node instanceof DOMElement ? $node : $node->documentElement);
            return ['document' => $xml, 'encrypted' => $encXml, 'session_key' => out_bytes($sym->key)];
        case 'enc_decrypt':
            $xml = isset($c['input']) ? in_bytes($c['input']) : run_scenario(['scenario' => 'enc_encrypt'] + $c['encrypt'])['document'];
            $doc = load_doc($xml, []);
            $enc = new XMLSecEnc();
            $data = $enc->locateEncryptedData($doc);
            if (!$data) {
                return ['located' => false];
            }
            $enc->setNode($data);
            $enc->type = $data->getAttribute('Type');
            $key = $enc->locateKey();
            $r = ['located' => true, 'type' => $enc->type, 'key_algorithm' => $key ? $key->getAlgorithm() : null];
            if (!$key) {
                return $r;
            }
            $info = $enc->locateKeyInfo($key);
            $r['key_info_encrypted'] = $info ? $info->isEncrypted : null;
            if ($info && $info->isEncrypted) {
                $info->loadKey(in_bytes($c['private_key']));
                $info->key = $info->encryptedCtx->decryptKey($info);
                $key = new_key(['alg' => $r['key_algorithm']]);
                $key->loadKey($info->key);
                $r['session_key_length'] = strlen($info->key);
            } elseif (isset($c['session_key'])) {
                $key->loadKey(in_bytes($c['session_key']));
            }
            $out = $enc->decryptNode($key, $c['replace'] ?? true);
            $r['result'] = $out instanceof DOMNode ? ($out instanceof DOMDocument ? $out->saveXML() : $doc->saveXML()) : out_bytes($out);
            return $r;
        case 'enc_key_info':
            $doc = load_doc(in_bytes($c['xml']), $c);
            $node = target($doc, $c['target']);
            $base = isset($c['base_key']) ? new_key($c['base_key']) : null;
            $k = XMLSecEnc::staticLocateKeyInfo($base, $node);
            if ($k === null) {
                return null;
            }
            return ['algorithm' => $k->getAlgorithm(), 'name' => $k->name, 'encrypted' => $k->isEncrypted, 'certificate' => $k->getX509Certificate(), 'thumbprint' => $k->getX509Thumbprint()];
        case 'x509':
            $cert = new X509Certificate(in_bytes($c['pfx']), in_bytes($c['password']));
            $date = static fn ($d) => $d instanceof DateTimeInterface ? $d->format('Y-m-d\TH:i:sP') : $d;
            return [
                'name' => $cert->getName(),
                'subject' => $cert->getSubject(),
                'issuer' => $cert->getIssuer(),
                'valid_from' => $date($cert->getValidFrom()),
                'expiration' => $date($cert->getExpiration()),
                'purposes' => $cert->getPurposes(),
                // Una extensión desconocida conserva sus bytes DER: puede no
                // ser UTF-8 válido.
                'extensions' => ($ext = $cert->getExtensions()) === null ? null : array_map('out_bytes', $ext),
                // json_decode en Rust ordena las claves: el orden se compara aparte.
                'extension_order' => $ext === null ? null : array_keys($ext),
                'public_key' => $cert->getPublicKey(),
                'private_key' => $cert->getPrivateKey(),
                'raw_equals_input' => $cert->getRaw() === in_bytes($c['pfx']),
                'export_pem' => $cert->export(X509ContentType::PEM),
                'export_cer' => $cert->export(X509ContentType::CER),
                'export_other' => $cert->export(99),
            ];
        default:
            throw new LogicException('escenario desconocido ' . $c['scenario']);
    }
}

function error_info(Throwable $e): array
{
    global $refDir, $keysDir;
    $message = str_replace([$refDir, (string) $keysDir], ['<ref>', '<keys>'], $e->getMessage());
    return ['class' => get_class($e), 'message' => $message];
}

$results = [];
foreach ($cases['cases'] as $case) {
    $warnings = [];
    // openssl_error_string() devuelve el error MÁS ANTIGUO pendiente del
    // proceso: se vacía la cola para que cada caso solo vea sus errores.
    while (openssl_error_string() !== false) {
    }
    try {
        $out = ['result' => run_scenario($case)];
    } catch (Throwable $e) {
        $out = ['error' => error_info($e)];
    }
    if (($case['openssl_detail_random'] ?? false) && isset($out['error'])) {
        $out['error']['message'] = preg_replace('/error:[0-9A-F]{8}:[^-]*/', '<detalle-openssl> ', $out['error']['message']);
    }
    if ($case['outcome_only'] ?? false) {
        // Resultado dependiente del azar: solo si se recuperó el original.
        $recovered = isset($out['result']) && (($out['result']['plaintext_matches'] ?? true) === true);
        $out = ['result' => ['original_recovered' => $recovered]];
    }
    $out['warnings'] = array_values(array_unique($warnings));
    $results[$case['id']] = $out;
}
echo json_encode([
    'generator' => 'oracle/php/xmldsig/run_cases.php',
    'reference' => ['tag' => $reference['baseline_tag'], 'commit' => $reference['baseline_commit']],
    'reference_version' => PHP_VERSION,
    'openssl' => OPENSSL_VERSION_TEXT,
    'libxml' => LIBXML_DOTTED_VERSION,
    'iconv_profile' => iconv_profile(),
    'libxml_cdata' => libxml_cdata(),
    'libxml_cr' => libxml_cr(),
] + array_filter(['libxml_xpath' => libxml_xpath($results)], static fn ($v) => $v !== null) + [
    'results' => $results,
], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
