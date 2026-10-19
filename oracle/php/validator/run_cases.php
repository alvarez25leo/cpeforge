<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Greenter\Validator\SymfonyValidator;

$input = $argv[1] ?? null;
if (!$input || !is_file($input)) {
    fwrite(STDERR, "Uso: php run_cases.php <cases.json>\n");
    exit(2);
}
$cases = json_decode(file_get_contents($input), true, 512, JSON_THROW_ON_ERROR);
$output = [];

function safe_value(mixed $value): mixed
{
    if (is_object($value)) {
        if ($value instanceof DateTimeInterface) {
            return $value->format('Y-m-d\TH:i:sP');
        }
        return ['type' => (new ReflectionClass($value))->getShortName()];
    }
    if (is_array($value)) {
        return array_map('safe_value', $value);
    }
    return $value;
}

function violation_result($violations): array
{
    $out = [];
    foreach ($violations as $violation) {
        $out[] = [
            'path' => $violation->getPropertyPath(),
            'message' => $violation->getMessage(),
            'template' => $violation->getMessageTemplate(),
            'code' => $violation->getCode(),
            'parameters' => safe_value($violation->getParameters()),
            'invalid' => safe_value($violation->getInvalidValue()),
        ];
    }
    return $out;
}

foreach ($cases as $case) {
    try {
        $validator = new SymfonyValidator();
        $validator->setVersion($case['version'] ?? null);
        $model = validator_hydrate($case['model']);
        $result = $validator->getValidator()->validate($model);
        $violations = violation_result($result);
        // Listas enormes (caso de límite de violaciones): cuántas son, la
        // primera y la última, igual que el ejecutor Rust.
        if (count($violations) > 1000) {
            $violations = ['$summary' => ['count' => count($violations), 'first' => $violations[0], 'last' => $violations[count($violations) - 1]]];
        }
        $output[] = ['id' => $case['id'], 'type' => $case['model']['type'], 'violations' => $violations];
    } catch (Throwable $error) {
        $message = preg_replace('/, called in .* on line \d+/', ', called by oracle', $error->getMessage());
        $output[] = ['id' => $case['id'], 'type' => $case['model']['type'], 'error' => [
            'type' => get_class($error),
            'message' => $message,
        ]];
    }
}
// PRESERVE_ZERO_FRACTION: un float entero (1e15) sigue siendo float en el
// fixture; sin él json_encode lo escribe como entero.
echo json_encode($output, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_PRESERVE_ZERO_FRACTION | JSON_THROW_ON_ERROR), "\n";
