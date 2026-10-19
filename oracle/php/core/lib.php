<?php

declare(strict_types=1);

/*
 * Funciones compartidas por los oráculos PHP: hidratación de casos neutrales
 * con los constructores y setters reales de Greenter y exportación de valores.
 * Ver la gramática de nodos en run_cases.php.
 */

require_once __DIR__ . '/bootstrap.php';

/** Métodos con lógica propia; se ejecutan como "calls", no como getters. */
const DERIVED_METHODS = ['getName', 'getXmlId', 'isAccepted'];

function fail(string $message): never
{
    fwrite(STDERR, $message . "\n");
    exit(2);
}

/** @return array<string, class-string> nombre corto -> FQCN */
function class_map(): array
{
    static $map = null;
    if ($map === null) {
        $map = [];
        foreach (greenter_core_symbols(GREENTER_CORE_SRC) as $fqcn) {
            $short = substr($fqcn, strrpos($fqcn, '\\') + 1);
            if (isset($map[$short])) {
                fail("nombre corto ambiguo: $short");
            }
            $map[$short] = $fqcn;
        }
    }
    return $map;
}

function hydrate(mixed $node): mixed
{
    if (!is_array($node)) {
        return $node;
    }
    if (array_is_list($node)) {
        return array_map('hydrate', $node);
    }
    if (array_key_exists('$decimal', $node)) {
        $text = $node['$decimal'];
        if (!is_string($text) || !preg_match('/^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/', $text)) {
            fail('$decimal inválido: ' . json_encode($text));
        }
        return (float) $text;
    }
    if (array_key_exists('$bytes', $node)) {
        $bytes = base64_decode((string) $node['$bytes'], true);
        if ($bytes === false) {
            fail('$bytes no es Base64 válido');
        }
        return $bytes;
    }
    if (array_key_exists('$datetime', $node)) {
        $class = $node['class'] ?? 'DateTimeImmutable';
        if (!in_array($class, ['DateTime', 'DateTimeImmutable'], true)) {
            fail("clase de fecha no soportada: $class");
        }
        $date = new $class($node['$datetime']);
        return $date->setTimezone(new DateTimeZone($node['tz']));
    }
    if (!array_key_exists('$class', $node)) {
        fail('objeto sin $class: ' . json_encode($node));
    }
    $map = class_map();
    $short = $node['$class'];
    if ($short === 'InvalidServiceResponseException') {
        $args = $node['$ctor'] ?? [];
        if (isset($args[2])) {
            $args[2] = hydrate($args[2]);
        }
        return new \Greenter\Services\InvalidServiceResponseException(...$args);
    }
    if ($short === 'LogicException') {
        return new LogicException(...($node['$ctor'] ?? []));
    }
    if (!isset($map[$short])) {
        fail("clase desconocida: $short");
    }
    $args = array_map('hydrate', $node['$ctor'] ?? []);
    $object = new ($map[$short])(...$args);
    foreach ($node as $key => $value) {
        if ($key === '$class' || $key === '$ctor') {
            continue;
        }
        $setter = 'set' . ucfirst($key);
        if (!method_exists($object, $setter)) {
            fail("setter inexistente: $short::$setter");
        }
        $object->$setter(hydrate($value));
    }
    return $object;
}

/** Elimina rutas locales y líneas del mensaje de error para que el fixture sea estable. */
function error_info(Throwable $e): array
{
    $message = preg_replace('/, called in .* on line \d+$/', '', $e->getMessage());
    $message = preg_replace('/ passed in .* on line \d+ / ', ' passed ', $message);
    return ['class' => get_class($e), 'message' => $message];
}

function short_name(object $object): string
{
    $class = get_class($object);
    $pos = strrpos($class, '\\');
    return $pos === false ? $class : substr($class, $pos + 1);
}

function export_value(mixed $value): mixed
{
    if ($value === null || is_bool($value) || is_int($value)) {
        return $value;
    }
    if (is_float($value)) {
        // json_encode no representa INF/NAN; se exportan como texto PHP.
        return ['$float' => is_finite($value) ? json_encode($value, JSON_PRESERVE_ZERO_FRACTION) : (string) $value];
    }
    if (is_string($value)) {
        return mb_check_encoding($value, 'UTF-8') ? $value : ['$bytes' => base64_encode($value)];
    }
    if (is_array($value)) {
        if (array_is_list($value)) {
            return array_map('export_value', $value);
        }
        return ['$map' => array_map('export_value', $value)];
    }
    if ($value instanceof DateTimeInterface) {
        return ['$datetime' => [
            'class' => get_class($value),
            'local' => $value->format('Y-m-d\TH:i:s.u'),
            'offset' => $value->getOffset(),
            'unix' => $value->getTimestamp(),
            'micro' => (int) $value->format('u'),
            'tz' => $value->getTimezone()->getName(),
        ]];
    }
    if ($value instanceof Throwable) {
        return [
            '$throwable' => get_class($value),
            'message' => $value->getMessage(),
            'code' => $value->getCode(),
            'previous' => export_value($value->getPrevious()),
        ];
    }
    if (is_object($value)) {
        return export_object($value);
    }
    fail('valor no exportable: ' . get_debug_type($value));
}

function export_object(object $object): array
{
    $out = ['$class' => short_name($object)];
    $methods = (new ReflectionClass($object))->getMethods(ReflectionMethod::IS_PUBLIC);
    usort($methods, static fn ($a, $b) => strcmp($a->getName(), $b->getName()));
    foreach ($methods as $method) {
        $name = $method->getName();
        if ($method->isStatic() || $method->getNumberOfRequiredParameters() > 0) {
            continue;
        }
        // getName() es derivado en los documentos; en DetailAttribute es un getter.
        if ($name === 'getName' ? $object instanceof \Greenter\Model\DocumentInterface : in_array($name, DERIVED_METHODS, true)) {
            continue;
        }
        if (str_starts_with($name, 'get')) {
            $key = lcfirst(substr($name, 3));
        } elseif (str_starts_with($name, 'is')) {
            $key = lcfirst(substr($name, 2));
        } else {
            continue;
        }
        try {
            $out[$key] = export_value($object->$name());
        } catch (Throwable $e) {
            $out[$key] = ['$error' => error_info($e)];
        }
    }
    return $out;
}

