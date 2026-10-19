<?php

declare(strict_types=1);

/*
 * Configuración de See y Api, como en https://greenter.dev/starter/ y en
 * thegreenter/demo (src/Util.php), pero sin credenciales en el código: se
 * leen de cert/ en la raíz del repositorio (o de las rutas en EJEMPLO_CERT y
 * EJEMPLO_CREDENCIALES).
 *
 * Los servicios quedan fijos en los entornos de prueba (SUNAT BETA y la API
 * GRE de GRE_API_URL): este ejemplo nunca usa producción.
 */

use Greenter\Api;
use Greenter\See;
use Greenter\Ws\Services\SunatEndpoints;

/**
 * Lee un texto con líneas `Clave: valor` o `CLAVE= valor`.
 *
 * @return array<string, string>
 */
function leer_credenciales(string $path): array
{
    $texto = @file_get_contents($path);
    if ($texto === false) {
        throw new RuntimeException("No se pudo leer el archivo de credenciales: $path");
    }
    $out = [];
    foreach (preg_split('/\R/u', $texto) as $linea) {
        if (preg_match('/^\s*([\p{L}_]+)\s*[:=]\s*(\S.*?)\s*$/u', $linea, $m)) {
            $out[$m[1]] ??= $m[2];
        }
    }
    foreach (['RUC', 'Usuario', 'Contraseña'] as $clave) {
        if (!isset($out[$clave])) {
            throw new RuntimeException("Falta «$clave:» en $path");
        }
    }

    return $out;
}

final class Config
{
    /** @param array<string, string> $cred */
    public function __construct(private string $pem, private array $cred)
    {
    }

    public function ruc(): string
    {
        return $this->cred['RUC'];
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
        $see->setClaveSOL($this->cred['RUC'], $this->cred['Usuario'], $this->cred['Contraseña']);

        return $see;
    }

    /** RUC del emisor de guías en la API GRE de prueba. */
    public function greRuc(): string
    {
        return $this->cred['GRE_RUC'] ?? '20161515648';
    }

    /** Api GRE de prueba; null si faltan sus credenciales. */
    public function api(): ?Api
    {
        foreach (['GRE_API_URL', 'GRE_CLIENT_ID', 'GRE_CLIENT_SECRET', 'GRE_RUC', 'GRE_USUARIO', 'GRE_CLAVE'] as $clave) {
            if (!isset($this->cred[$clave])) {
                return null;
            }
        }
        $url = $this->cred['GRE_API_URL'];
        if (str_contains($url, 'sunat.gob.pe') && !str_contains($url, 'test') && !str_contains($url, 'beta')) {
            throw new RuntimeException('GRE_API_URL apunta a producción; este ejemplo solo usa entornos de prueba');
        }
        $api = new Api(['auth' => $url, 'cpe' => $url]);

        return $api->setApiCredentials($this->cred['GRE_CLIENT_ID'], $this->cred['GRE_CLIENT_SECRET'])
            ->setClaveSOL($this->cred['GRE_RUC'], $this->cred['GRE_USUARIO'], $this->cred['GRE_CLAVE'])
            ->setCertificate($this->pem);
    }
}

$raiz = dirname(__DIR__, 2);
$certPath = getenv('EJEMPLO_CERT') ?: $raiz.'/cert/certificate.pem';
$credPath = getenv('EJEMPLO_CREDENCIALES') ?: $raiz.'/cert/user.txt';

$pem = @file_get_contents($certPath);
if ($pem === false) {
    throw new RuntimeException("No se pudo leer el certificado: $certPath");
}

return new Config($pem, leer_credenciales($credPath));
