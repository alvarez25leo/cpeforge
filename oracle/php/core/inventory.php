<?php

declare(strict_types=1);

/*
 * Inventario por reflexión de la API pública de greenter/core.
 * Uso: php oracle/php/core/inventory.php > tests/reference/core/inventory.json
 */

require_once __DIR__ . '/bootstrap.php';

function type_name(?ReflectionType $type): ?string
{
    return $type === null ? null : (string) $type;
}

function doc_tag(string|false $doc, string $tag): ?string
{
    if ($doc === false || !preg_match('/@' . $tag . '\s+([^\s*]+)/', $doc, $m)) {
        return null;
    }
    return $m[1];
}

function method_body(ReflectionMethod $m): string
{
    $lines = file($m->getFileName());
    $src = implode('', array_slice($lines, $m->getStartLine() - 1, $m->getEndLine() - $m->getStartLine() + 1));
    return trim($src);
}

/** Clasifica cuerpos triviales; los demás se conservan literalmente para revisión. */
function classify(ReflectionMethod $m): array
{
    $body = method_body($m);
    $inner = preg_replace('/\s+/', ' ', $body);
    if (preg_match('/^public function (?:get|is)(\w+)\(\)(?:: \??[\w\\\\]+)? \{ return \$this->(\w+); \}$/', $inner, $g)) {
        return ['kind' => 'getter', 'property' => $g[2]];
    }
    if (preg_match('/^public function set(\w+)\((?:\??[\w\\\\]+ )?\$(\w+)\)(?:: \??[\w\\\\]+)? \{ \$this->(\w+) = \$(\w+); return \$this; \}$/', $inner, $s)
        && $s[2] === $s[4]) {
        return ['kind' => 'setter', 'property' => $s[3]];
    }
    return ['kind' => 'custom', 'source' => $body];
}

$out = [
    // Solo tag y commit: la URL y el clon local no forman parte del fixture.
    'reference' => array_intersect_key(
        json_decode((string) file_get_contents(dirname(__DIR__, 3) . '/oracle/upstream/referencia.json'), true),
        ['baseline_tag' => true, 'baseline_commit' => true]
    ),
    'reference_version' => PHP_VERSION,
    'classes' => [],
];

foreach (greenter_core_symbols(GREENTER_CORE_SRC) as $name) {
    $r = new ReflectionClass($name);
    $kind = $r->isInterface() ? 'interface' : ($r->isTrait() ? 'trait' : ($r->isAbstract() ? 'abstract class' : ($r->isFinal() ? 'final class' : 'class')));
    $defaults = $r->isInterface() || $r->isTrait() ? [] : $r->getDefaultProperties();
    $props = [];
    foreach ($r->getProperties() as $p) {
        if ($p->getDeclaringClass()->getName() !== $name) {
            continue;
        }
        $props[] = [
            'name' => $p->getName(),
            'visibility' => $p->isPublic() ? 'public' : ($p->isProtected() ? 'protected' : 'private'),
            'type' => type_name($p->getType()),
            'doc_var' => doc_tag($p->getDocComment(), 'var'),
            'has_default' => $p->hasDefaultValue(),
            'default' => $defaults[$p->getName()] ?? null,
        ];
    }
    $methods = [];
    foreach ($r->getMethods() as $m) {
        if ($m->getDeclaringClass()->getName() !== $name) {
            continue;
        }
        $params = [];
        foreach ($m->getParameters() as $p) {
            $params[] = [
                'name' => $p->getName(),
                'type' => type_name($p->getType()),
                'optional' => $p->isOptional(),
                'default' => $p->isDefaultValueAvailable() ? $p->getDefaultValue() : null,
            ];
        }
        $entry = [
            'name' => $m->getName(),
            'visibility' => $m->isPublic() ? 'public' : ($m->isProtected() ? 'protected' : 'private'),
            'static' => $m->isStatic(),
            'params' => $params,
            'return_type' => type_name($m->getReturnType()),
            'doc_return' => doc_tag($m->getDocComment(), 'return'),
        ];
        if (!$r->isInterface()) {
            $entry += classify($m);
        }
        $methods[] = $entry;
    }
    $out['classes'][$name] = [
        'file' => substr($r->getFileName(), strlen(GREENTER_CORE_SRC) + 1),
        'kind' => $kind,
        'parent' => $r->getParentClass() ? $r->getParentClass()->getName() : null,
        'interfaces' => array_values(array_diff($r->getInterfaceNames(), $r->getParentClass() ? $r->getParentClass()->getInterfaceNames() : [])),
        'traits' => $r->getTraitNames(),
        'constants' => $r->getConstants(),
        'properties' => $props,
        'methods' => $methods,
    ];
}

echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_PRESERVE_ZERO_FRACTION), "\n";
