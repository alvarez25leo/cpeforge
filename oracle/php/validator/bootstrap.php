<?php

declare(strict_types=1);

$root = dirname(__DIR__, 3);
$reference = $root . '/target/reference/v5.3.0';
$suite = $root . '/target/reference/v5.3.0-suite';
if (!is_dir($reference . '/packages/validator/src') || !is_file($suite . '/vendor/autoload.php')) {
    fwrite(STDERR, "Referencia PHP o dependencias faltantes\n");
    exit(2);
}
require $suite . '/vendor/autoload.php';
// El vendor de Composer proviene de la suite aislada, pero Greenter se carga
// siempre desde el checkout exacto indicado por la tarea.
spl_autoload_register(static function (string $class) use ($reference): void {
    if (!str_starts_with($class, 'Greenter\\')) {
        return;
    }
    $relative = substr($class, strlen('Greenter\\'));
    foreach ([$reference . '/packages/validator/src', $reference . '/packages/core/src/Core'] as $source) {
        $path = $source . '/' . str_replace('\\', '/', $relative) . '.php';
        if (is_file($path)) {
            require $path;
            return;
        }
    }
}, true, true);
date_default_timezone_set('America/Lima');

function validator_class(string $name): string
{
    static $classes = null;
    if ($classes === null) {
        $classes = [];
        foreach (new RecursiveIteratorIterator(new RecursiveDirectoryIterator(
            dirname(__DIR__, 3) . '/target/reference/v5.3.0/packages/core/src/Core/Model',
            FilesystemIterator::SKIP_DOTS
        )) as $file) {
            if ($file->getExtension() !== 'php') {
                continue;
            }
            $source = file_get_contents($file->getPathname());
            if (!preg_match('/namespace\s+([^;]+);/', $source, $namespace) ||
                !preg_match('/\b(?:class|trait|interface)\s+(\w+)/', $source, $symbol)) {
                continue;
            }
            $classes[$symbol[1]] = $namespace[1] . '\\' . $symbol[1];
        }
    }
    if (!isset($classes[$name])) {
        throw new RuntimeException("Modelo desconocido: $name");
    }
    return $classes[$name];
}

/**
 * Expande entradas grandes descritas de forma compacta en cases.json (los
 * casos de límites): {"$repeat": {"text": "A", "count": n}} es una cadena,
 * {"$repeat": {"value": v, "count": n}} una lista de n copias de v y
 * {"$nest": {"depth": n, "leaf": v}} n listas anidadas. El ejecutor Rust
 * (validator_oracle) aplica la misma expansión.
 */
function validator_expand(mixed $value): mixed
{
    if (!is_array($value)) {
        return $value;
    }
    if (isset($value['$repeat']) && count($value) === 1) {
        $spec = $value['$repeat'];
        return isset($spec['text'])
            ? str_repeat($spec['text'], $spec['count'])
            : array_fill(0, $spec['count'], validator_expand($spec['value']));
    }
    if (isset($value['$nest']) && count($value) === 1) {
        $nested = validator_expand($value['$nest']['leaf']);
        for ($i = 0; $i < $value['$nest']['depth']; $i++) {
            $nested = [$nested];
        }
        return $nested;
    }
    return array_map('validator_expand', $value);
}

function validator_hydrate(array $spec): object
{
    $spec['fields'] = validator_expand($spec['fields'] ?? []);
    $class = validator_class($spec['type']);
    $reflection = new ReflectionClass($class);
    $constructor = $reflection->getConstructor();
    $object = $constructor === null || $constructor->getNumberOfRequiredParameters() === 0
        ? $reflection->newInstance()
        : $reflection->newInstanceWithoutConstructor();
    foreach ($spec['fields'] ?? [] as $field => $value) {
        $setter = 'set' . ucfirst($field);
        if (!method_exists($object, $setter)) {
            throw new RuntimeException("Setter ausente: $class::$setter");
        }
        if (is_array($value) && array_key_exists('type', $value)) {
            $value = validator_hydrate($value);
        } elseif (is_array($value) && array_is_list($value)) {
            $value = array_map(static fn ($item) => is_array($item) && isset($item['type']) ? validator_hydrate($item) : $item, $value);
        } elseif (is_array($value) && isset($value['$date'])) {
            $value = new DateTime($value['$date']);
        }
        if ($spec['raw'] ?? false) {
            $property = new ReflectionProperty($class, $field);
            $property->setValue($object, $value);
        } else {
            $object->$setter($value);
        }
    }
    return $object;
}
