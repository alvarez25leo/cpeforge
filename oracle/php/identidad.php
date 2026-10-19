<?php
declare(strict_types=1);

/*
 * Identidad CPEForge para los oráculos que firman de extremo a extremo.
 *
 * CPEForge firma con `Id="CPEForgeSign"` y referencia `#CPEFORGE-SIGN` en
 * `cac:Signature`. Esa URI está dentro del documento firmado, así que cambia
 * el digest, la firma y el ZIP: traducir la salida no basta. Este archivo
 * registra un autoloader (con prioridad) que carga dos clases de la
 * referencia con esos dos identificadores sustituidos, sin modificar el
 * checkout fijado:
 *
 * - Greenter\XMLSecLibs\XMLSecurityDSig: BASE_TEMPLATE con Id="CPEForgeSign".
 * - Greenter\Xml\Builder\TwigBuilder: render() devuelve el XML con
 *   #CPEFORGE-SIGN.
 *
 * Requiere que el autoloader de Composer de la suite ya esté registrado.
 * Uso: require este archivo después de los autoloaders del oráculo.
 */

(static function (): void {
    $patches = [
        'Greenter\\XMLSecLibs\\XMLSecurityDSig' => static fn (string $src): string =>
            identidad_reemplazar($src, 'Id="GreenterSign"', 'Id="CPEForgeSign"'),
        'Greenter\\Xml\\Builder\\TwigBuilder' => static fn (string $src): string =>
            identidad_reemplazar(
                $src,
                "return \$this->twig->render(\$template, [\n            'doc' => \$doc,\n        ]);",
                "return str_replace('#GREENTER-SIGN', '#CPEFORGE-SIGN', \$this->twig->render(\$template, [\n            'doc' => \$doc,\n        ]));"
            ),
    ];
    $dir = sys_get_temp_dir().'/cpeforge-identidad-'.getmypid();
    spl_autoload_register(static function (string $class) use ($patches, $dir): void {
        if (!isset($patches[$class])) {
            return;
        }
        $file = identidad_buscar($class);
        $patched = $dir.'/'.str_replace('\\', '_', $class).'.php';
        if (!is_dir($dir) && !mkdir($dir, 0700, true) && !is_dir($dir)) {
            throw new RuntimeException("no se pudo crear $dir");
        }
        // __DIR__/__FILE__ deben seguir apuntando al archivo original (plantillas).
        $src = strtr((string) file_get_contents($file), [
            '__DIR__' => var_export(dirname($file), true),
            '__FILE__' => var_export($file, true),
        ]);
        file_put_contents($patched, $patches[$class]($src));
        require $patched;
        register_shutdown_function(static function () use ($patched, $dir): void {
            @unlink($patched);
            @rmdir($dir);
        });
    }, true, true);
})();

function identidad_reemplazar(string $src, string $from, string $to): string
{
    if (substr_count($src, $from) !== 1) {
        throw new RuntimeException('identidad: el código de la referencia cambió; revisar oracle/php/identidad.php');
    }
    return str_replace($from, $to, $src);
}

/**
 * Archivo de la clase sin cargarla: primero los paquetes de la referencia
 * fijada (como el autoloader de los oráculos), después Composer.
 */
function identidad_buscar(string $class): string
{
    $ref = dirname(__DIR__, 2).'/target/reference/v5.3.0/packages';
    $relative = str_replace('\\', '/', substr($class, strlen('Greenter\\'))).'.php';
    foreach (['lite/src/Greenter', 'xml/src', 'ws/src', 'core/src/Core'] as $part) {
        if (is_file("$ref/$part/$relative")) {
            return "$ref/$part/$relative";
        }
    }
    foreach (spl_autoload_functions() as $loader) {
        if (is_array($loader) && $loader[0] instanceof Composer\Autoload\ClassLoader) {
            $file = $loader[0]->findFile($class);
            if (is_string($file)) {
                return $file;
            }
        }
    }
    throw new RuntimeException("identidad: no se encontró $class");
}
