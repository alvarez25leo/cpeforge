<?php

declare(strict_types=1);

/*
 * Datos compartidos por los casos. Equivalen a example/rust/src/datos.rs.
 *
 * - `Contexto`: la fecha base y los RUC de las credenciales. Las fechas
 *   relativas del demo (`new DateTime('-3days')`) se calculan desde la fecha
 *   base, para que PHP y Rust firmen el mismo XML.
 * - `empresa()` y `cliente()`: copia de `Greenter\Data\SharedStore` de
 *   greenter/data v5.3.0, que usa el demo (sus otros datos no los usa ningún caso).
 */

use Greenter\Model\Client\Client;
use Greenter\Model\Company\Address;
use Greenter\Model\Company\Company;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\SaleDetail;

final class Contexto
{
    public function __construct(
        private DateTime $fecha,
        public readonly string $ruc,
        public readonly string $greRuc,
    ) {
    }

    /** Fecha base (hora de Lima). Cada llamada devuelve una copia. */
    public function fecha(): DateTime
    {
        return clone $this->fecha;
    }

    /** `new DateTime('+N days')` del demo, desde la fecha base. */
    public function dias(int $dias): DateTime
    {
        return $this->fecha()->modify(sprintf('%+d days', $dias));
    }
}

function empresa(): Company
{
    return (new Company())
        ->setRuc('20123456789')
        ->setNombreComercial('GREENTER')
        ->setRazonSocial('GREENTER S.A.C.')
        ->setAddress((new Address())
            ->setUbigueo('150101')
            ->setDistrito('LIMA')
            ->setProvincia('LIMA')
            ->setDepartamento('LIMA')
            ->setUrbanizacion('CASUARINAS')
            ->setCodLocal('0000')
            ->setDireccion('AV NEW DEÁL 123'))
        ->setEmail('admin@greenter.com')
        ->setTelephone('01-234455');
}

function cliente(): Client
{
    return (new Client())
        ->setTipoDoc('6')
        ->setNumDoc('20000000001')
        ->setRznSocial('EMPRESA 1 S.A.C.')
        ->setAddress((new Address())
            ->setDireccion('JR. NIQUEL MZA. F LOTE. 3 URB.  INDUSTRIAL INFAÑTAS - LIMA - LIMA -PERU'))
        ->setEmail('client@corp.com')
        ->setTelephone('01-445566');
}

/** Emisor de las guías (`Util::getGRECompany()` del demo). */
function empresa_gre(Contexto $c): Company
{
    return (new Company())
        ->setRuc($c->greRuc)
        ->setRazonSocial('GREENTER S.A.C.');
}

/*
 * Casos de inicio (https://greenter.dev/starter/ y /examples/exonerada/),
 * emitidos con el RUC de las credenciales.
 */

function inicio_emisor(string $ruc): Company
{
    $address = (new Address())
        ->setUbigueo('150101')
        ->setDepartamento('LIMA')
        ->setProvincia('LIMA')
        ->setDistrito('LIMA')
        ->setUrbanizacion('-')
        ->setDireccion('Av. Villa Nueva 221')
        ->setCodLocal('0000'); // Código de establecimiento SUNAT, 0000 por defecto.

    return (new Company())
        ->setRuc($ruc)
        ->setRazonSocial('GREEN SAC')
        ->setNombreComercial('GREEN')
        ->setAddress($address)
        ->setEmail('ventas@green.example')
        ->setTelephone('01-2345678');
}

function inicio_cliente_empresa(): Client
{
    return (new Client())
        ->setTipoDoc('6') // RUC - Catálogo 06
        ->setNumDoc('20123456789')
        ->setRznSocial('EMPRESA X S.A.C.')
        ->setAddress((new Address())->setDireccion('Jr. Niquel 453, San Miguel - Lima'))
        ->setEmail('compras@empresax.example')
        ->setTelephone('01-4455667');
}

function inicio_cliente_persona(): Client
{
    return (new Client())
        ->setTipoDoc('1') // DNI - Catálogo 06
        ->setNumDoc('48285071')
        ->setRznSocial('NIPAO GUVI')
        ->setAddress((new Address())->setDireccion('Calle Fusión 453, San Miguel - Lima'));
}

/** Línea gravada (catálogo 07: 10) con IGV 18 %. */
function inicio_linea_gravada(string $codigo, string $descripcion, float $cantidad, float $valorUnitario, float $valorVenta, float $igv, float $precioUnitario): SaleDetail
{
    return (new SaleDetail())
        ->setCodProducto($codigo)
        ->setUnidad('NIU') // Unidad - Catálogo 03
        ->setCantidad($cantidad)
        ->setMtoValorUnitario($valorUnitario)
        ->setDescripcion($descripcion)
        ->setMtoBaseIgv($valorVenta)
        ->setPorcentajeIgv(18.00)
        ->setIgv($igv)
        ->setTipAfeIgv('10') // Gravado - Operación onerosa
        ->setTotalImpuestos($igv)
        ->setMtoValorVenta($valorVenta)
        ->setMtoPrecioUnitario($precioUnitario);
}

/** Bolsa plástica gravada con ICBPER (S/ 0.50 por bolsa desde 2023). */
function inicio_linea_bolsa(float $cantidad, float $valorVenta, float $igv, float $icbper): SaleDetail
{
    return (new SaleDetail())
        ->setCodProducto('BOLSA')
        ->setUnidad('NIU')
        ->setCantidad($cantidad)
        ->setDescripcion('BOLSA DE PLASTICO')
        ->setMtoValorUnitario(0.50)
        ->setMtoPrecioUnitario(0.59)
        ->setMtoValorVenta($valorVenta)
        ->setTipAfeIgv('10')
        ->setMtoBaseIgv($valorVenta)
        ->setPorcentajeIgv(18.00)
        ->setIgv($igv)
        ->setIcbper($icbper) // cantidad × factor ICBPER
        ->setFactorIcbper(0.50)
        ->setTotalImpuestos($igv + $icbper);
}

function inicio_invoice(string $tipoDoc, string $serie, string $correlativo, DateTime $fecha, Company $company, Client $client): Invoice
{
    return (new Invoice())
        ->setUblVersion('2.1')
        ->setTipoOperacion('0101') // Venta interna - Catálogo 51
        ->setTipoDoc($tipoDoc) // 01 factura, 03 boleta - Catálogo 01
        ->setSerie($serie)
        ->setCorrelativo($correlativo)
        ->setFechaEmision(clone $fecha)
        ->setTipoMoneda('PEN') // Sol - Catálogo 02
        ->setCompany($company)
        ->setClient($client);
}
