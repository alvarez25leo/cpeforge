<?php

declare(strict_types=1);

/*
 * F001-3: factura a crédito en dos cuotas con líneas gravada, inafecta,
 * gratuita y bolsa plástica (ICBPER). Total 172.36.
 */

use Greenter\Model\Sale\Cuota;
use Greenter\Model\Sale\Document;
use Greenter\Model\Sale\FormaPagos\FormaPagoCredito;
use Greenter\Model\Sale\Invoice;
use Greenter\Model\Sale\Legend;
use Greenter\Model\Sale\SaleDetail;

return static function (Contexto $c): Invoice {
    $inafecta = (new SaleDetail())
        ->setCodProducto('P003')
        ->setCodProdSunat('10191509') // Catálogo 25 (UNSPSC)
        ->setUnidad('NIU')
        ->setCantidad(2)
        ->setDescripcion('LIBRO IMPRESO')
        ->setMtoValorUnitario(25.00)
        ->setMtoValorVenta(50.00)
        ->setMtoBaseIgv(50.00)
        ->setPorcentajeIgv(0)
        ->setIgv(0)
        ->setTipAfeIgv('30') // Inafecto - Operación onerosa
        ->setTotalImpuestos(0)
        ->setMtoPrecioUnitario(25.00);

    $gratuita = (new SaleDetail())
        ->setCodProducto('P004')
        ->setUnidad('NIU')
        ->setCantidad(1)
        ->setDescripcion('MUESTRA GRATIS')
        ->setMtoValorUnitario(0)
        ->setMtoValorGratuito(20.00)
        ->setMtoValorVenta(20.00)
        ->setMtoBaseIgv(20.00)
        ->setPorcentajeIgv(18)
        ->setIgv(3.60)
        ->setTipAfeIgv('11') // Gravado - Retiro por premio (gratuito)
        ->setTotalImpuestos(3.60)
        ->setMtoPrecioUnitario(0);

    $cuota1 = (clone $c->fecha())->modify('+30 days')->setTime(0, 0);
    $cuota2 = (clone $c->fecha())->modify('+60 days')->setTime(0, 0);

    return inicio_invoice('01', 'F001', '3', $c->fecha(), inicio_emisor($c->ruc), inicio_cliente_empresa())
        ->setFormaPago(new FormaPagoCredito(172.36))
        ->setCuotas([
            (new Cuota())->setMonto(86.18)->setFechaPago($cuota1),
            (new Cuota())->setMonto(86.18)->setFechaPago($cuota2),
        ])
        ->setCompra('OC-2024-0015')
        ->setObservacion('FACTURA DE EJEMPLO A CREDITO')
        ->setGuias([(new Document())->setTipoDoc('09')->setNroDoc('T001-00000015')])
        ->setMtoOperGravadas(102.00)
        ->setMtoOperInafectas(50.00)
        ->setMtoOperGratuitas(20.00)
        ->setMtoIGVGratuitas(3.60)
        ->setMtoIGV(18.36)
        ->setIcbper(2.00)
        ->setTotalImpuestos(20.36)
        ->setValorVenta(152.00)
        ->setSubTotal(172.36)
        ->setMtoImpVenta(172.36)
        ->setDetails([
            inicio_linea_gravada('P001', 'PRODUCTO 1', 2, 50.00, 100.00, 18.00, 59.00),
            $inafecta,
            $gratuita,
            inicio_linea_bolsa(4, 2.00, 0.36, 2.00),
        ])
        ->setLegends([
            (new Legend())->setCode('1000')->setValue('SON CIENTO SETENTA Y DOS CON 36/100 SOLES'),
            (new Legend())->setCode('1002')->setValue('TRANSFERENCIA GRATUITA DE UN BIEN Y/O SERVICIO PRESTADO GRATUITAMENTE'),
        ]);
};
