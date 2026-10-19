<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Greenter\Validator\Metadata\CustomMetadataFactory;

$root = dirname(__DIR__, 3);
$source = $root . '/target/reference/v5.3.0/packages/validator/src/Validator';
$out = ['reference' => 'f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f', 'symfony' => '6.4.48', 'files' => [], 'profiles' => []];
$it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($source, FilesystemIterator::SKIP_DOTS));
foreach ($it as $file) {
    if ($file->getExtension() === 'php') {
        $path = substr($file->getPathname(), strlen($source) + 1);
        $out['files'][$path] = hash_file('sha256', $file->getPathname());
    }
}
ksort($out['files']);

foreach (['default' => null, '2.1' => '2.1', 'unknown' => '9.9'] as $profile => $version) {
    $factory = new CustomMetadataFactory();
    $factory->setVersion($version);
    $out['profiles'][$profile] = [];
    foreach (['default', '2.1'] as $loaderProfile) {
        $dir = $source . '/Loader' . ($loaderProfile === '2.1' ? '/v21' : '');
        foreach (glob($dir . '/*Loader.php') as $path) {
            $name = basename($path, 'Loader.php');
            $class = validator_class($name === 'Document' ? 'Document' : $name);
            $object = (new ReflectionClass($class))->newInstanceWithoutConstructor();
            $metadata = $factory->getMetadataFor($object);
            $fields = [];
            foreach ($metadata->getConstrainedProperties() as $field) {
                $rules = [];
                foreach ($metadata->getPropertyMetadata($field) as $propertyMetadata) {
                    foreach ($propertyMetadata->getConstraints() as $constraint) {
                        $options = [];
                        foreach (get_object_vars($constraint) as $key => $value) {
                            if (in_array($key, ['groups', 'payload'], true) || $value === null) {
                                continue;
                            }
                            if (is_scalar($value) || is_array($value)) {
                                $options[$key] = $value;
                            }
                        }
                        $rules[] = ['name' => (new ReflectionClass($constraint))->getShortName(), 'options' => $options];
                    }
                    if ($propertyMetadata->getCascadingStrategy() === \Symfony\Component\Validator\Mapping\CascadingStrategy::CASCADE) {
                        $rules[] = ['name' => 'Valid', 'options' => []];
                    }
                }
                $fields[$field] = $rules;
            }
            $out['profiles'][$profile][$name] = [
                'loader' => $version === '2.1' && is_file($source . '/Loader/v21/' . $name . 'Loader.php') ? 'v21' : 'base',
                'fields' => (object) $fields,
                'has_metadata' => $factory->hasMetadataFor($object),
            ];
        }
    }
    ksort($out['profiles'][$profile]);
}

echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
