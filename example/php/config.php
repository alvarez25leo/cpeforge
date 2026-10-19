<?php

declare(strict_types=1);

/*
 * Configuración de See y Api, como en https://greenter.dev/starter/ y en
 * thegreenter/demo (src/Util.php), pero sin credenciales en el código: se
 * leen de example/credenciales/credenciales-prueba.json (o del JSON en
 * EJEMPLO_CREDENCIALES). El certificado es el del campo `certificado`,
 * relativo al JSON, o el de EJEMPLO_CERT.
 *
 * Los servicios quedan fijos en los entornos de prueba (SUNAT BETA y la API
 * GRE de gre.api_url): este ejemplo nunca usa producción.
 */

use Greenter\Api;
use Greenter\See;
use Greenter\Ws\Services\SunatEndpoints;

/**
 * Lee el JSON de credenciales y comprueba los campos obligatorios.
 *
 * @return array{certificado: string, sunat_beta: array<string, string>, gre?: array<string, string>}
 */
function leer_credenciales(string $path): array
{
    $texto = @file_get_contents($path);
    if ($texto === false) {
        throw new RuntimeException("No se pudo leer el archivo de credenciales: $path");
    }
    $json = json_decode($texto, true, 16, JSON_THROW_ON_ERROR);
    $campos = ['sunat_beta' => ['ruc', 'usuario', 'clave']];
    if (isset($json['gre'])) {
        $campos['gre'] = ['api_url', 'client_id', 'client_secret', 'ruc', 'usuario', 'clave'];
    }
    foreach ($campos as $objeto => $claves) {
        foreach ($claves as $clave) {
            if (!is_string($json[$objeto][$clave] ?? null) || $json[$objeto][$clave] === '') {
                throw new RuntimeException("Falta «$objeto.$clave» en $path");
            }
        }
    }
    if (!is_string($json['certificado'] ?? null)) {
        throw new RuntimeException("Falta «certificado» en $path");
    }

    return $json;
}

final class Config
{
    /** @param array{sunat_beta: array<string, string>, gre?: array<string, string>} $cred */
    public function __construct(private string $pem, private array $cred)
    {
    }

    public function ruc(): string
    {
        return $this->cred['sunat_beta']['ruc'];
    }

    /** See para facturas, boletas, notas, resúmenes y bajas (FE_BETA). */
    public function see(): See
    {
        return $this->seeEn(SunatEndpoints::FE_BETA);
    }

    /** See para percepción, retención y reversión (RETENCION_BETA). */
    public function seeRetencion(): See
    {
        return $this->seeEn(SunatEndpoints::RETENCION_BETA);
    }

    private function seeEn(string $endpoint): See
    {
        $see = new See();
        $see->setCertificate($this->pem);
        $see->setService($endpoint);
        $sol = $this->cred['sunat_beta'];
        $see->setClaveSOL($sol['ruc'], $sol['usuario'], $sol['clave']);

        return $see;
    }

    /** RUC del emisor de guías en la API GRE de prueba. */
    public function greRuc(): string
    {
        return $this->cred['gre']['ruc'] ?? '20161515648';
    }

    /** Api GRE de prueba; null si el JSON no tiene la sección gre. */
    public function api(): ?Api
    {
        $gre = $this->cred['gre'] ?? null;
        if ($gre === null) {
            return null;
        }
        $url = $gre['api_url'];
        if (str_contains($url, 'sunat.gob.pe') && !str_contains($url, 'test') && !str_contains($url, 'beta')) {
            throw new RuntimeException('gre.api_url apunta a producción; este ejemplo solo usa entornos de prueba');
        }
        $api = new Api(['auth' => $url, 'cpe' => $url]);

        return $api->setApiCredentials($gre['client_id'], $gre['client_secret'])
            ->setClaveSOL($gre['ruc'], $gre['usuario'], $gre['clave'])
            ->setCertificate($this->pem);
    }
}

$raiz = dirname(__DIR__, 2);
$credPath = getenv('EJEMPLO_CREDENCIALES') ?: $raiz.'/example/credenciales/credenciales-prueba.json';
$cred = leer_credenciales($credPath);
$certPath = getenv('EJEMPLO_CERT') ?: dirname($credPath).'/'.$cred['certificado'];

$pem = @file_get_contents($certPath);
if ($pem === false) {
    throw new RuntimeException("No se pudo leer el certificado: $certPath");
}

return new Config($pem, $cred);
