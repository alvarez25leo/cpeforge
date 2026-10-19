<?php

declare(strict_types=1);

require __DIR__ . '/bootstrap.php';

use Greenter\Validator\ErrorCodeProviderInterface;
use Greenter\Validator\MessageTranslator;
use Greenter\Validator\Metadata\LoaderListenerInterface;
use Greenter\Validator\SymfonyValidator;
use Symfony\Component\Validator\Constraints\Callback;
use Symfony\Component\Validator\Constraints\NotBlank;
use Symfony\Component\Validator\Context\ExecutionContextInterface;
use Symfony\Component\Validator\Mapping\ClassMetadata;

class SyntheticProvider implements ErrorCodeProviderInterface
{
    public function getAll(): ?array { return ['G001' => 'Proveedor', '1003' => '']; }
    public function getValue(?string $code): ?string { return $this->getAll()[$code] ?? null; }
}

class EmptyProvider implements ErrorCodeProviderInterface
{
    public function getAll(): ?array { return []; }
    public function getValue(?string $code): ?string { return ''; }
}

class SyntheticListener implements LoaderListenerInterface
{
    public function onLoaded(object $value, ClassMetadata $metadata): void
    {
        if ($value instanceof \Greenter\Model\Sale\Invoice) {
            $metadata->addConstraint(new Callback([$this, 'validate']));
        }
    }
    public function validate(object $value, ExecutionContextInterface $context): void
    {
        $context->buildViolation('Regla sintética')->atPath('fechaEmision')->addViolation();
    }
}

class FixedDateListener implements LoaderListenerInterface
{
    public function onLoaded(object $value, ClassMetadata $metadata): void
    {
        if ($value instanceof \Greenter\Model\Sale\Invoice) {
            $metadata->addConstraint(new Callback([$this, 'validate']));
        }
    }
    public function validate(object $value, ExecutionContextInterface $context): void
    {
        $now = new DateTimeImmutable('2025-01-10T00:00:00-05:00');
        $date = $value->getFechaEmision();
        if ($date !== null && $now->diff($date)->days > 7) {
            $context->buildViolation('Fecha emision superó 7 dias')->atPath('fechaEmision')->addViolation();
        }
    }
}

class PropertyListener implements LoaderListenerInterface
{
    public function onLoaded(object $value, ClassMetadata $metadata): void
    {
        if ($value instanceof \Greenter\Model\Client\Client) {
            $metadata->addPropertyConstraint('email', new NotBlank(['message' => 'Email obligatoria']));
        }
    }
}

$provider = new SyntheticProvider();
$translator = new MessageTranslator($provider);
$translator->addResource(['G001' => 'Recurso', '1003' => 'Recurso 1003']);
$translatorResults = [];
foreach (['G001', '1003', 'desconocido', ''] as $code) {
    $translatorResults[$code] = $translator->trans($code, ['{{ value }}' => 'ABC'], 'validators', 'es_PE');
}
$fallbackTranslator = new MessageTranslator(new EmptyProvider());
$fallbackTranslator->addResource(['G001' => 'Recurso G001', '1003' => 'Recurso 1003']);

$validator = new SymfonyValidator();
$invoice = new \Greenter\Model\Sale\Invoice();
$stages = [];
foreach ([['default', null, false], ['2.1', '2.1', false], ['unknown', '9.9', false], ['listener', '2.1', true], ['default-again', null, true]] as [$name, $version, $listener]) {
    $validator->setVersion($version);
    if ($listener) {
        $validator->getMetadatFactory()->setListener(new SyntheticListener());
    }
    $stages[$name] = array_map(static fn ($v) => [$v->getPropertyPath(), $v->getMessage()], iterator_to_array($validator->validate($invoice)));
}
$factory = $validator->getMetadatFactory();
$unknown = new stdClass();
$dateCallback = [];
foreach (['2025-01-01', '2025-01-07', '2025-01-20'] as $day) {
    $dateValidator = new SymfonyValidator();
    $dateValidator->getMetadatFactory()->setListener(new FixedDateListener());
    $datedInvoice = (new \Greenter\Model\Sale\Invoice())->setFechaEmision(new DateTime($day . 'T00:00:00-05:00'));
    $dateCallback[$day] = array_values(array_filter(
        array_map(static fn ($v) => [$v->getPropertyPath(), $v->getMessage()], iterator_to_array($dateValidator->validate($datedInvoice))),
        static fn ($item) => $item[1] === 'Fecha emision superó 7 dias'
    ));
}
$propertyValidator = new SymfonyValidator();
$propertyValidator->getMetadatFactory()->setListener(new PropertyListener());
$emptyClient = new \Greenter\Model\Client\Client();
$propertyMetadata = $propertyValidator->getMetadatFactory()->getMetadataFor($emptyClient);
$propertyExtension = [
    'has_email' => in_array('email', $propertyMetadata->getConstrainedProperties(), true),
    'violations' => array_map(
        static fn ($v) => [$v->getPropertyPath(), $v->getMessage()],
        iterator_to_array($propertyValidator->getValidator()->validate($emptyClient))
    ),
];
$out = [
    'translation' => $translatorResults,
    'translation_empty_provider' => [
        'G001' => $fallbackTranslator->trans('G001'),
        '1003' => $fallbackTranslator->trans('1003'),
        'unknown' => $fallbackTranslator->trans('unknown'),
    ],
    'locale' => $translator->getLocale(),
    'metadata' => [
        'invoice' => $factory->hasMetadataFor($invoice),
        'stdClass' => $factory->hasMetadataFor($unknown),
        'unknown_constraints' => count($factory->getMetadataFor($unknown)->getConstraints()),
        'unknown_properties' => $factory->getMetadataFor($unknown)->getConstrainedProperties(),
    ],
    'stages' => $stages,
    'date_callback' => $dateCallback,
    'property_extension' => $propertyExtension,
];
echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
