# Matriz de validator 0.1.0 frente a Greenter PHP v5.3.0

Fuente fijada: `thegreenter/greenter` `f24e546d3e62e0ce4c2ebfbe23c7e8c9d464d51f`; Symfony Validator 6.4.48. Generar con `python3 scripts/build_validator_matrix.py`. El inventario JSON guarda el SHA-256 de cada fuente, campos, orden, opciones y versiones; `cases.json` y `reference-results.json` conservan cada entrada, salida y error por ID.

## API y metadatos públicos

| Fuente PHP | Entrada y comportamiento | Salida/error | Equivalente/consumidor |
| --- | --- | --- | --- |
| `SymfonyValidator.php` | `validate(DocumentInterface)`, `setVersion`, `getValidator`, `getMetadatFactory`; proveedor opcional | Violaciones ordenadas; error de tipo PHP | `Validator::validate_document`, `validate_model`, `set_version`, `metadata_for`; consumidores de `core` |
| `MessageTranslator.php` | `trans(id, parámetros, dominio, locale)`; proveedor/recurso G001/fallback | Mensaje o ID; fallo del proveedor | `MessageTranslator`, `Validator::translate`; mensajes de restricciones y `xcodes` futuro |
| `Metadata/CustomMetadataFactory.php` | Clase, versión, listener | Metadata efectiva, `hasMetadataFor`, listener tras carga | `metadata_for`, `has_metadata_for`, `RuleExtension`; todos los loaders |
| `Metadata/LoaderMetadataInterface.php` | Objeto de clase registrada | Restricciones de clase y propiedad | Tabla `rules.json`; `Validator` |
| `Metadata/LoaderListenerInterface.php` | Metadata recién cargada | Reglas añadidas y callbacks | `RuleExtension`; consumidor `Validator` |

## Los 40 loaders (33 base y siete UBL 2.1)

Cada caso usa el modelo `cpeforge-core` correspondiente, salvo los casos `raw` que ejercitan Symfony sin pasar por setters. La salida de éxito es una lista de violaciones con ruta, mensaje, plantilla, código, parámetros y valor rechazado; el fallo del setter es `TypeError`. Los contadores de error señalan las diferencias de mensaje entre PHP y Rust. `rules.json` conserva todas las opciones.

| Fuente PHP | Perfil efectivo | Modelo y consumidor | Campos/reglas | Restricciones | Casos PHP / TypeError |
| --- | --- | --- | ---: | --- | ---: |
| `Loader/AddressLoader.php` | default | `Address` → Client, Company | 8/9 | Length, NotBlank | default: 47/4, 2.1: 47/4 |
| `Loader/ClientLoader.php` | default | `Client` → Invoice, Note, Despatch, Perception, Retention | 4/7 | Length, NotBlank, Valid | default: 46/6, 2.1: 46/6 |
| `Loader/CompanyLoader.php` | default | `Company` → Invoice, Note, Despatch, Summary, Voided, Retention, Perception | 4/6 | Length, NotBlank, Regex, Valid | default: 30/4, 2.1: 30/4 |
| `Loader/CuotaLoader.php` | default | `Cuota` → Invoice, Note | 3/3 | Length, NotNull | default: 11/0, 2.1: 11/0 |
| `Loader/DespatchDetailLoader.php` | default | `DespatchDetail` → Despatch | 5/6 | Length, NotNull, Required | default: 20/0, 2.1: 20/0 |
| `Loader/DespatchLoader.php` | default | `Despatch` → raíz o `Validator::validate_model` | 12/19 | Length, NotBlank, NotNull, Valid | default: 67/14, 2.1: 67/14 |
| `Loader/DetractionLoader.php` | default | `Detraction` → raíz o `Validator::validate_model` | 2/2 | NotNull | default: 3/0 |
| `Loader/DirectionLoader.php` | default | `Direction` → Shipment, EmbededDespatch | 2/4 | Length, NotBlank | default: 27/4, 2.1: 27/4 |
| `Loader/DocumentLoader.php` | default | `Document` → Invoice, Note, Despatch, SummaryDetail | 2/2 | Length | default: 13/0, 2.1: 13/0 |
| `Loader/EmbededDespatchLoader.php` | default | `EmbededDespatch` → Invoice | 10/10 | Length, Type, Valid | default: 47/1, 2.1: 47/1 |
| `Loader/ExchangeLoader.php` | default | `Exchange` → RetentionDetail, PerceptionDetail | 4/4 | NotBlank, NotNull | default: 21/4, 2.1: 21/4 |
| `Loader/FormaPagoContadoLoader.php` | default | `FormaPagoContado` → raíz o `Validator::validate_model` | 1/1 | NotBlank | default: 10/2, 2.1: 10/2 |
| `Loader/FormaPagoCreditoLoader.php` | default | `FormaPagoCredito` → raíz o `Validator::validate_model` | 3/3 | Length, NotBlank, NotNull | default: 19/2, 2.1: 19/2 |
| `Loader/InvoiceLoader.php` | default | `Invoice` → raíz o `Validator::validate_model` | 19/23 | Choice, NotBlank, NotNull, Regex, Valid | default: 61/8 |
| `Loader/LegendLoader.php` | default | `Legend` → Invoice, Note | 2/2 | Length | default: 13/0, 2.1: 13/0 |
| `Loader/NoteLoader.php` | default | `Note` → raíz o `Validator::validate_model` | 20/29 | Length, NotBlank, NotNull, Valid | default: 128/16 |
| `Loader/PaymentLoader.php` | default | `Payment` → RetentionDetail, PerceptionDetail | 3/4 | Length, NotBlank, NotNull | default: 28/9, 2.1: 28/9 |
| `Loader/PaymentTermsLoader.php` | default | `PaymentTerms` → raíz o `Validator::validate_model` | 2/2 | Length, NotBlank | default: 18/2, 2.1: 18/2 |
| `Loader/PerceptionDetailLoader.php` | default | `PerceptionDetail` → Perception | 10/13 | Length, NotBlank, NotNull, Valid | default: 52/14, 2.1: 52/14 |
| `Loader/PerceptionLoader.php` | default | `Perception` → raíz o `Validator::validate_model` | 11/17 | Length, NotBlank, NotNull, Valid | default: 69/14, 2.1: 69/14 |
| `Loader/PrepaymentLoader.php` | default | `Prepayment` → Invoice | 3/5 | Length, NotBlank, NotNull | default: 32/4, 2.1: 32/4 |
| `Loader/RetentionDetailLoader.php` | default | `RetentionDetail` → Retention | 10/14 | Length, NotBlank, NotNull, Valid | default: 52/14, 2.1: 52/14 |
| `Loader/RetentionLoader.php` | default | `Retention` → raíz o `Validator::validate_model` | 11/17 | Length, NotBlank, NotNull, Valid | default: 69/14, 2.1: 69/14 |
| `Loader/ReversionLoader.php` | default | `Reversion` → raíz o `Validator::validate_model` | 5/8 | Length, NotBlank, NotNull, Valid | default: 31/10, 2.1: 31/10 |
| `Loader/SaleDetailLoader.php` | default | `SaleDetail` → Invoice, Note | 10/11 | Length, NotBlank, NotNull | default: 37/4 |
| `Loader/SalePerceptionLoader.php` | default | `SalePerception` → Invoice, Note | 4/7 | NotBlank, NotNull, Type | default: 58/5 |
| `Loader/ShipmentLoader.php` | default | `Shipment` → Despatch | 12/15 | Length, NotBlank, Type, Valid | default: 96/23, 2.1: 96/23 |
| `Loader/SummaryDetailLoader.php` | default | `SummaryDetail` → Summary | 9/9 | Length, NotBlank, NotNull, Valid | default: 40/6, 2.1: 40/6 |
| `Loader/SummaryLoader.php` | default | `Summary` → raíz o `Validator::validate_model` | 6/9 | Length, NotBlank, NotNull, Valid | default: 40/12, 2.1: 40/12 |
| `Loader/SummaryPerceptionLoader.php` | default | `SummaryPerception` → SummaryDetail | 5/6 | Choice, NotBlank, NotNull | default: 16/2, 2.1: 16/2 |
| `Loader/TransportistLoader.php` | default | `Transportist` → Shipment | 6/12 | Length, NotBlank | default: 87/12, 2.1: 87/12 |
| `Loader/VoidedDetailLoader.php` | default | `VoidedDetail` → Voided, Reversion | 4/6 | Length, NotBlank, Regex | default: 43/8, 2.1: 43/8 |
| `Loader/VoidedLoader.php` | default | `Voided` → raíz o `Validator::validate_model` | 5/8 | Length, NotBlank, NotNull, Valid | default: 32/10, 2.1: 32/10 |
| `Loader/v21/ChargeLoader.php` | 2.1 | `Charge` → Invoice, SaleDetail | 4/4 | NotBlank, NotNull | 2.1: 13/2 |
| `Loader/v21/DetailAttributeLoader.php` | 2.1 | `DetailAttribute` → SaleDetail | 2/2 | NotBlank | 2.1: 19/4 |
| `Loader/v21/DetractionLoader.php` | 2.1 | `Detraction` → raíz o `Validator::validate_model` | 5/5 | NotBlank, NotNull | 2.1: 30/6 |
| `Loader/v21/InvoiceLoader.php` | 2.1 | `Invoice` → raíz o `Validator::validate_model` | 23/28 | Length, NotBlank, NotNull, Valid | 2.1: 79/10 |
| `Loader/v21/NoteLoader.php` | 2.1 | `Note` → raíz o `Validator::validate_model` | 20/25 | Length, NotBlank, NotNull, Valid | 2.1: 100/16 |
| `Loader/v21/SaleDetailLoader.php` | 2.1 | `SaleDetail` → Invoice, Note | 16/17 | Length, NotBlank, NotNull, Valid | 2.1: 62/13 |
| `Loader/v21/SalePerceptionLoader.php` | 2.1 | `SalePerception` → Invoice, Note | 5/5 | NotBlank, NotNull | 2.1: 14/2 |

En el perfil base, `Charge` y `DetailAttribute` no tienen loader: `hasMetadataFor` es falso y la validación devuelve cero violaciones. La versión desconocida `9.9` recae en el perfil base. Los casos de ausencia y cambio de versión están en `cases.json` y `api-probe.json`.

## Suite PHP auditada

Doce clases `*Test.php` y cuatro archivos de apoyo; ejecutadas con salida de red denegada por `sandbox-exec`. Los nombres se toman del checkout fijado:

- `Validator/CeDespatchValidatorTest.php` — clase de prueba.
- `Validator/CePerceptionValidatorTest.php` — clase de prueba.
- `Validator/CeRetentionValidatorTest.php` — clase de prueba.
- `Validator/CeReversionValidatorTest.php` — clase de prueba.
- `Validator/CustomValidator.php` — apoyo.
- `Validator/FeInvoice21ValidatorTest.php` — clase de prueba.
- `Validator/FeInvoiceValidatorTest.php` — clase de prueba.
- `Validator/FeNote21ValidatorTest.php` — clase de prueba.
- `Validator/FeNoteValidatorTest.php` — clase de prueba.
- `Validator/FeSummaryValidatorTest.php` — clase de prueba.
- `Validator/FeVoidedValidatorTest.php` — clase de prueba.
- `Validator/Listener/InvoiceListener.php` — apoyo.
- `Validator/NonDocumentTest.php` — clase de prueba.
- `Validator/SymfonyValidatorTest.php` — clase de prueba.
- `Validator/Validator21Trait.php` — apoyo.
- `Validator/ValidatorTrait.php` — apoyo.

Total de casos: 2761, con 476 `TypeError` PHP. El runner/comparador recorre todos los IDs en orden; `scripts/check_validator_rules.py` comprueba opciones completas, cobertura por campo y bordes independientes.
