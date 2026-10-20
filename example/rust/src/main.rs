//! Genera y firma los casos de `casos/` con los crates Rust.
//!
//! ```text
//! cargo run                         # todos los casos: XML firmado en out/
//! cargo run -- factura boleta       # solo esos casos
//! cargo run -- --enviar [casos]     # además los envía a los entornos de prueba
//! cargo run -- --errores [casos]    # casos de rechazo en out/errores/
//! ```
//!
//! `EJEMPLO_FECHA`
//! (`AAAA-MM-DD HH:MM:SS`, hora de Lima) fija la fecha base; sin ella se usa
//! la hora actual. Con la misma fecha, el XML firmado es reproducible.
//!
//! Envío según el documento:
//! - factura, boleta, nota: `sendBill` a SUNAT BETA y CDR;
//! - percepción, retención: `sendBill` al servicio BETA de retenciones;
//! - resumen, baja: `sendSummary` a SUNAT BETA, ticket y `getStatus`;
//! - reversión: igual, en el servicio BETA de retenciones;
//! - guía: API GRE de prueba (`gre.api_url` del JSON de credenciales), ticket y estado.
//!
//! SUNAT BETA responde `401` a los envíos muy seguidos: entre uno y otro se
//! esperan `EJEMPLO_PAUSA` segundos (5 por defecto).
//!
//! Benchmark: cada fase de cada caso se mide y queda en `out/benchmark.json`,
//! que `oracle/ejemplos/benchmark.py` puede contrastar con otra implementación:
//! - construir (el modelo), xml (el builder), firma (`SignedXml`) y see
//!   (`See::get_xml_signed` completo): mediana de `EJEMPLO_REPETICIONES`
//!   llamadas (10 por defecto) en microsegundos, y la primera llamada de `see`;
//! - envio (`See`/`Api::send`) y estado (las consultas de ticket, sin las
//!   pausas): una medición en milisegundos, solo con `--enviar`.
//!
//! Para medir, compilar en modo release (`cargo run --release`).

mod casos;
mod config;
mod datos;
mod errores;

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use cpeforge::SendResult;
use cpeforge_core::Zoned;
use cpeforge_core::model::response::{CdrResponse, ResponseError, StatusResult};
use cpeforge_xmldsig::sunat::SignedXml;
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use jiff::{RoundMode, Unit, ZonedRound};

use casos::{CASOS, Documento};
use config::Config;
use datos::Contexto;
use errores::CASOS_ERROR;

/// Casos que solo generan XML: su envío original va a un servidor de terceros.
const SOLO_XML: &[&str] = &["factura-beatose"];

fn fecha_base() -> Result<Zoned, Box<dyn Error>> {
    let lima = TimeZone::get("America/Lima")?;
    match std::env::var("EJEMPLO_FECHA") {
        Ok(texto) => {
            let civil = DateTime::strptime("%Y-%m-%d %H:%M:%S", &texto)
                .map_err(|_| "EJEMPLO_FECHA debe tener el formato AAAA-MM-DD HH:MM:SS")?;
            Ok(civil.to_zoned(lima)?)
        }
        // Sin fracciones de segundo: el XML solo lleva segundos, como la referencia.
        Err(_) => Ok(Zoned::now().with_time_zone(lima).round(
            ZonedRound::new()
                .smallest(Unit::Second)
                .mode(RoundMode::Trunc),
        )?),
    }
}

/// Mensaje del error y de sus causas (`a: b: c`).
fn cadena(error: &dyn Error) -> String {
    let mut texto = error.to_string();
    let mut causa = error.source();
    while let Some(e) = causa {
        texto.push_str(&format!(": {e}"));
        causa = e.source();
    }
    texto
}

/// Tiempos de un caso (ver `out/benchmark.json`).
#[derive(Default)]
struct Medida {
    construir_us: f64,
    xml_us: f64,
    firma_us: f64,
    see_us: f64,
    see_primera_us: f64,
    envio_ms: Option<f64>,
    estado_ms: Option<f64>,
    resultado: Option<String>,
    codigo: Option<String>,
    mensaje: Option<String>,
    observaciones: Vec<String>,
}

/// Llama a `f` `veces` y devuelve el último resultado, la mediana y la
/// primera llamada, en microsegundos.
fn medir<T, E>(veces: usize, mut f: impl FnMut() -> Result<T, E>) -> Result<(T, f64, f64), E> {
    let mut tiempos = Vec::with_capacity(veces);
    let mut resultado = None;
    for _ in 0..veces.max(1) {
        let inicio = Instant::now();
        resultado = Some(f()?);
        tiempos.push(inicio.elapsed().as_secs_f64() * 1e6);
    }
    let primera = tiempos.first().copied().unwrap_or_default();
    tiempos.sort_by(f64::total_cmp);
    let medio = tiempos.len() / 2;
    let centro = tiempos.get(medio).copied().unwrap_or_default();
    let mediana = if tiempos.len() % 2 == 1 {
        centro
    } else {
        (tiempos
            .get(medio.wrapping_sub(1))
            .copied()
            .unwrap_or(centro)
            + centro)
            / 2.0
    };
    match resultado {
        Some(resultado) => Ok((resultado, mediana, primera)),
        // `veces.max(1)` garantiza al menos una llamada.
        None => f().map(|r| (r, mediana, primera)),
    }
}

/// Microsegundos legibles: «850 µs», «12.4 ms».
fn us(us: f64) -> String {
    if us < 1000.0 {
        format!("{us:.0} µs")
    } else {
        format!("{:.1} ms", us / 1000.0)
    }
}

fn milisegundos(desde: Instant) -> f64 {
    desde.elapsed().as_secs_f64() * 1e3
}

fn pausa() -> Duration {
    Duration::from_secs(
        std::env::var("EJEMPLO_PAUSA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
    )
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let enviar = args.iter().any(|a| a == "--enviar");
    let errores = args.iter().any(|a| a == "--errores");
    let casos = if errores { CASOS_ERROR } else { CASOS };
    let filtro: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| *a != "--enviar" && *a != "--errores")
        .collect();
    for nombre in &filtro {
        if !casos.iter().any(|(caso, _)| caso == nombre) {
            return Err(format!("Caso desconocido: {nombre}").into());
        }
    }

    let config = Config::cargar()?;
    let ctx = Contexto::new(fecha_base()?, config.ruc(), config.gre_ruc());
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("out")
        .join(if errores { "errores" } else { "" });
    fs::create_dir_all(&out)?;

    let mut see = config.see()?;
    let repeticiones: usize = std::env::var("EJEMPLO_REPETICIONES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
        .max(1);
    let mut nombres: HashMap<String, &str> = HashMap::new();
    let mut enviados = 0;
    let mut fallos = 0;
    let mut benchmark = Vec::new();
    for (caso, construir) in casos {
        if !filtro.is_empty() && !filtro.contains(caso) {
            continue;
        }
        let mut medida = Medida::default();
        let (doc, t, _) = medir(repeticiones, || construir(&ctx))?;
        medida.construir_us = t;
        let name = doc.as_ref().name()?;
        if let Some(otro) = nombres.insert(name.clone(), caso) {
            return Err(format!("{caso} y {otro} generan el mismo archivo {name}").into());
        }
        let (xml, t, primera) = medir(repeticiones, || {
            see.get_xml_signed(doc.as_ref()).map(str::to_owned)
        })?;
        (medida.see_us, medida.see_primera_us) = (t, primera);
        // Las fases por separado, con el builder y el firmador que See dejó listos.
        let factory = see.factory_ref();
        let builder = factory.builder().ok_or("See sin builder")?;
        let signer = factory.signer().ok_or("See sin firmador")?;
        let (sin_firma, t, _) = medir(repeticiones, || builder.build(doc.as_ref()))?;
        medida.xml_us = t;
        let (firmado, t, _) = medir(repeticiones, || signer.sign_xml(&sin_firma))?;
        medida.firma_us = t;
        if firmado != xml {
            return Err(format!("{caso}: builder + firma no reproducen get_xml_signed").into());
        }
        fs::write(out.join(format!("{name}.xml")), &xml)?;

        // Comprobación independiente: la firma se verifica con el
        // certificado incluido en el propio XML.
        let valida = SignedXml::new().verify_xml(&xml)?;
        println!(
            "{caso}: {name}.xml ({} bytes), firma {}",
            xml.len(),
            if valida { "válida" } else { "INVÁLIDA" }
        );
        if !valida {
            fallos += 1;
        }
        println!(
            "  tiempos: construir {} · xml {} · firma {} · get_xml_signed {} (primera {})",
            us(medida.construir_us),
            us(medida.xml_us),
            us(medida.firma_us),
            us(medida.see_us),
            us(medida.see_primera_us)
        );

        if enviar && SOLO_XML.contains(caso) {
            println!("  sin envío: el demo lo envía a un servidor de terceros");
        } else if enviar {
            if enviados > 0 {
                tokio::time::sleep(pausa()).await;
            }
            enviados += 1;
            match enviar_documento(&config, &doc, &xml, &out, &mut medida).await {
                Ok(aceptado) if aceptado == errores => fallos += 1,
                Ok(_) => {}
                Err(error) => {
                    println!("  ERROR: {}", cadena(error.as_ref()));
                    medida.resultado = Some("error".to_owned());
                    fallos += 1;
                }
            }
            println!(
                "  tiempos de red: envío {:.0} ms · estado {}",
                medida.envio_ms.unwrap_or_default(),
                medida
                    .estado_ms
                    .map_or_else(|| "-".to_owned(), |ms| format!("{ms:.0} ms"))
            );
        }
        benchmark.push(serde_json::json!({
            "caso": caso,
            "archivo": format!("{name}.xml"),
            "construir_us": medida.construir_us,
            "xml_us": medida.xml_us,
            "firma_us": medida.firma_us,
            "see_us": medida.see_us,
            "see_primera_us": medida.see_primera_us,
            "envio_ms": medida.envio_ms,
            "estado_ms": medida.estado_ms,
            "resultado": medida.resultado,
            "codigo": medida.codigo,
            "mensaje": medida.mensaje,
            "observaciones": medida.observaciones,
        }));
    }

    let total = |campo: &str| -> f64 { benchmark.iter().filter_map(|m| m[campo].as_f64()).sum() };
    println!(
        "Total (medianas): construir {} · xml {} · firma {} · get_xml_signed {}",
        us(total("construir_us")),
        us(total("xml_us")),
        us(total("firma_us")),
        us(total("see_us"))
    );
    let informe = serde_json::json!({
        "lenguaje": "rust",
        "perfil": if cfg!(debug_assertions) { "debug" } else { "release" },
        "repeticiones": repeticiones,
        "enviar": enviar,
        "errores": errores,
        "casos": benchmark,
    });
    fs::write(
        out.join("benchmark.json"),
        serde_json::to_string_pretty(&informe)? + "\n",
    )?;

    if fallos > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Envía según el tipo de documento e informa el resultado.
async fn enviar_documento(
    config: &Config,
    doc: &Documento,
    xml: &str,
    out: &Path,
    medida: &mut Medida,
) -> Result<bool, Box<dyn Error>> {
    let name = doc.as_ref().name()?;
    medida.envio_ms = Some(0.0);

    if let Documento::Despatch(_) = doc {
        let Some(mut api) = config.api()? else {
            println!("  sin envío: falta la sección gre en el JSON de credenciales");
            medida.resultado = Some("sin envío".to_owned());
            return Ok(true);
        };
        let inicio = Instant::now();
        let result = api.send(doc.as_ref()).await?;
        medida.envio_ms = Some(milisegundos(inicio));
        if api.last_xml() != Some(xml) {
            println!("  aviso: el XML enviado difiere del generado antes");
        }
        if result.base.success != Some(true) {
            return Ok(error(result.base.error, medida));
        }
        let ticket = result.ticket.unwrap_or_default();
        println!("  ticket: {ticket}");
        let mut status = StatusResult::default();
        let mut consultas = 0.0;
        for _ in 0..5 {
            tokio::time::sleep(pausa()).await;
            let inicio = Instant::now();
            status = api.get_status(&ticket).await?;
            consultas += milisegundos(inicio);
            if status.code.as_deref() != Some("98") {
                break;
            }
            println!("  en proceso…");
        }
        medida.estado_ms = Some(consultas);
        return estado(status, &name, out, medida);
    }

    let retencion = matches!(
        doc,
        Documento::Perception(_) | Documento::Retention(_) | Documento::Reversion(_)
    );
    let mut see = if retencion {
        config.see_retencion()?
    } else {
        config.see()?
    };
    let inicio = Instant::now();
    let result = see.send(doc.as_ref()).await?;
    medida.envio_ms = Some(milisegundos(inicio));
    if see.factory_ref().last_xml() != Some(xml) {
        println!("  aviso: el XML enviado difiere del generado antes");
    }
    match result {
        SendResult::Bill(bill) => {
            if bill.base.success != Some(true) {
                return Ok(error(bill.base.error, medida));
            }
            if let Some(zip) = &bill.cdr_zip {
                fs::write(out.join(format!("R-{name}.zip")), zip)?;
            }
            Ok(cdr(bill.cdr_response, medida))
        }
        SendResult::Summary(summary) => {
            if summary.base.success != Some(true) {
                return Ok(error(summary.base.error, medida));
            }
            let ticket = summary.ticket.unwrap_or_default();
            println!("  ticket: {ticket}");
            // El ticket puede quedar «en proceso» (98) unos segundos.
            let mut status = StatusResult::default();
            let mut consultas = 0.0;
            for _ in 0..5 {
                tokio::time::sleep(pausa()).await;
                let inicio = Instant::now();
                status = see.get_status(&ticket).await?;
                consultas += milisegundos(inicio);
                if status.code.as_deref() != Some("98") {
                    break;
                }
                println!("  en proceso…");
            }
            medida.estado_ms = Some(consultas);
            estado(status, &name, out, medida)
        }
        // `SendResult` admite variantes nuevas en versiones futuras.
        _ => Err("resultado de envío no admitido por este ejemplo".into()),
    }
}

fn estado(
    status: StatusResult,
    name: &str,
    out: &Path,
    medida: &mut Medida,
) -> Result<bool, Box<dyn Error>> {
    if status.bill.base.success != Some(true) {
        return Ok(error(status.bill.base.error, medida));
    }
    if let Some(zip) = &status.bill.cdr_zip {
        fs::write(out.join(format!("R-{name}.zip")), zip)?;
    }
    Ok(cdr(status.bill.cdr_response, medida))
}

fn error(error: Option<ResponseError>, medida: &mut Medida) -> bool {
    let error = error.unwrap_or_default();
    let code = error.code.unwrap_or_default();
    let message = error.message.unwrap_or_default();
    println!("  ERROR {code}: {message}");
    medida.codigo = Some(code.clone());
    medida.mensaje = Some(message);
    medida.resultado = Some(format!("error {code}"));
    false
}

/// Interpreta el CDR devuelto por SUNAT.
fn cdr(cdr: Option<CdrResponse>, medida: &mut Medida) -> bool {
    let cdr = cdr.unwrap_or_default();
    medida.codigo = cdr.code.clone();
    medida.mensaje = cdr.description.clone();
    medida.observaciones = cdr.notes.clone().unwrap_or_default();
    let code: u32 = cdr
        .code
        .as_deref()
        .unwrap_or("")
        .trim()
        .parse()
        .unwrap_or(u32::MAX);
    let resultado = match code {
        0 => "aceptada",
        2000..=3999 => "rechazada",
        _ => "excepción",
    };
    println!("  ESTADO: {}", resultado.to_uppercase().replace('Ó', "O"));
    medida.resultado = Some(resultado.to_owned());
    println!("  {}", cdr.description.unwrap_or_default());
    for nota in cdr.notes.unwrap_or_default() {
        println!("  observación: {nota}");
    }
    code == 0
}
