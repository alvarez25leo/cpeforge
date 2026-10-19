<?php

declare(strict_types=1);

/*
 * Carga `greenter/core` desde la copia aislada de la referencia fijada.
 * `greenter/core` no tiene dependencias Composer, por lo que basta un
 * autoloader PSR-4 equivalente al de su composer.json.
 */

$referenceDir = getenv('GREENTER_PHP_REFERENCE') ?: dirname(__DIR__, 3) . '/target/reference/v5.3.0';
$coreSrc = realpath($referenceDir . '/packages/core/src/Core');
if ($coreSrc === false) {
    fwrite(STDERR, "No existe la referencia PHP en $referenceDir; ejecutar scripts/prepare_php_reference.sh\n");
    exit(2);
}

spl_autoload_register(static function (string $class) use ($coreSrc): void {
    $prefix = 'Greenter\\';
    if (strncmp($class, $prefix, strlen($prefix)) !== 0) {
        return;
    }
    $file = $coreSrc . '/' . str_replace('\\', '/', substr($class, strlen($prefix))) . '.php';
    if (is_file($file)) {
        require $file;
    }
});

/** @return list<string> Nombres de clase de todos los archivos del paquete. */
function greenter_core_symbols(string $coreSrc): array
{
    $symbols = [];
    $it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($coreSrc, FilesystemIterator::SKIP_DOTS));
    foreach ($it as $file) {
        if ($file->getExtension() !== 'php') {
            continue;
        }
        $rel = substr($file->getPathname(), strlen($coreSrc) + 1, -4);
        $symbols[] = 'Greenter\\' . str_replace('/', '\\', $rel);
    }
    sort($symbols);
    return $symbols;
}

define('GREENTER_CORE_SRC', $coreSrc);
