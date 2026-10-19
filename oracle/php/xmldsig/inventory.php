<?php

declare(strict_types=1);

/*
 * Inventario por reflexión de la API pública de greenter/xmldsig v5.1.0:
 * clases de src/ con sus constantes, propiedades y métodos públicos, y las
 * pruebas .phpt de la referencia.
 *
 * Uso: php oracle/php/xmldsig/inventory.php > inventario.json
 */

$root = dirname(__DIR__, 3);
// El aviso de deprecación de PHP 8.4 en SignedXml no es parte del inventario.
set_error_handler(static fn (): bool => true, E_DEPRECATED);
$refDir = getenv('XMLDSIG_PHP_REFERENCE') ?: $root . '/target/reference/xmldsig-v5.1.0';
require $refDir . '/vendor/autoload.php';

$files = [];
$it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($refDir . '/src'));
foreach ($it as $f) {
    if ($f->isFile() && $f->getExtension() === 'php') {
        $files[] = substr($f->getPathname(), strlen($refDir) + 1);
    }
}
sort($files);

$classes = [];
foreach ($files as $file) {
    $src = (string) file_get_contents($refDir . '/' . $file);
    preg_match('/namespace\s+([^;]+);/', $src, $ns);
    preg_match('/(?:abstract\s+)?class\s+(\w+)/', $src, $cl);
    $fqcn = $ns[1] . '\\' . $cl[1];
    $r = new ReflectionClass($fqcn);
    $entry = ['file' => $file, 'class' => $fqcn, 'abstract' => $r->isAbstract(), 'constants' => [], 'properties' => [], 'methods' => []];
    foreach ($r->getReflectionConstants() as $c) {
        if ($c->isPublic()) {
            $entry['constants'][] = $c->getName();
        }
    }
    foreach ($r->getProperties(ReflectionProperty::IS_PUBLIC) as $p) {
        $entry['properties'][] = ($p->isStatic() ? 'static ' : '') . '$' . $p->getName();
    }
    foreach ($r->getMethods(ReflectionMethod::IS_PUBLIC) as $m) {
        if ($m->getDeclaringClass()->getName() !== $fqcn) {
            continue;
        }
        $params = array_map(static fn (ReflectionParameter $p) => '$' . $p->getName() . ($p->isOptional() ? '?' : ''), $m->getParameters());
        $entry['methods'][] = ($m->isStatic() ? 'static ' : '') . $m->getName() . '(' . implode(', ', $params) . ')';
    }
    $classes[] = $entry;
}

$tests = [];
foreach (glob($refDir . '/tests/*.phpt') as $t) {
    preg_match('/--TEST--\s*(.+)/', (string) file_get_contents($t), $m);
    $tests[] = ['file' => 'tests/' . basename($t), 'title' => trim($m[1] ?? '')];
}

echo json_encode(['source_files' => $files, 'classes' => $classes, 'tests' => $tests], JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE), "\n";
