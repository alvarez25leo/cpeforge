//! Pruebas de seguridad, límites y errores de `cpeforge-report`.
//!
//! Complementan la paridad (`parity.rs`): rutas de plantillas, HTML no
//! confiable, `|raw`, límites de logo, plantilla y combustible, y XML
//! malicioso. Ninguna prueba ejecuta comandos ni usa red.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "pruebas: un fallo debe abortar la prueba"
)]

use std::fs;

use cpeforge_core::Decimal;
use cpeforge_core::model::client::Client;
use cpeforge_core::model::company::Company;
use cpeforge_core::model::sale::{BaseSale, Invoice, SaleDetail};
use cpeforge_report::{
    Extra, HtmlReport, RenderCause, ReportError, ReportOptions, ReportParameters, TrustedHtml,
    XmlUtils, XmlUtilsError,
};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn invoice() -> Invoice {
    Invoice {
        base: BaseSale {
            tipo_doc: Some("01".into()),
            serie: Some("F001".into()),
            correlativo: Some("1".into()),
            fecha_emision: Some("2024-03-15T10:00:00-05:00[America/Lima]".parse().unwrap()),
            tipo_moneda: Some("PEN".into()),
            company: Some(Company {
                ruc: Some("20000000001".into()),
                razon_social: Some("EMISOR <b>&</b>".into()),
                ..Company::default()
            }),
            client: Some(Client {
                tipo_doc: Some("6".into()),
                num_doc: Some("20000000002".into()),
                rzn_social: Some("<script>alert('x')</script>".into()),
                ..Client::default()
            }),
            mto_igv: Some(Decimal::new(1800, 2)),
            mto_imp_venta: Some(Decimal::new(11800, 2)),
            details: Some(vec![SaleDetail {
                descripcion: Some("<img src=x onerror=alert(1)>".into()),
                cantidad: Some(Decimal::ONE),
                mto_valor_unitario: Some(Decimal::new(100, 0)),
                mto_valor_venta: Some(Decimal::new(100, 0)),
                ..SaleDetail::default()
            }]),
            ..BaseSale::default()
        },
        ..Invoice::default()
    }
}

fn params() -> ReportParameters {
    let mut p = ReportParameters::default();
    p.system.logo = Some(PNG.to_vec());
    p
}

#[test]
fn untrusted_text_is_escaped_and_only_trusted_html_is_raw() {
    let mut p = params();
    p.user.header = Some(TrustedHtml::new("<b>cabecera</b>"));
    p.user.footer = Some(TrustedHtml::escape("<i>pie</i> & 'x'"));
    p.user.extras = Some(vec![Extra::new("<u>n</u>", "<script>v</script>")]);
    p.system.hash = Some("\"><script>h</script>".into());
    let html = HtmlReport::default().render_html(&invoice(), &p).unwrap();
    assert!(html.contains("<b>cabecera</b>"));
    assert!(html.contains("&lt;i&gt;pie&lt;/i&gt; &amp; &#039;x&#039;"));
    assert!(html.contains("&lt;script&gt;alert(&#039;x&#039;)&lt;/script&gt;"));
    assert!(html.contains("&lt;img src=x onerror=alert(1)&gt;"));
    assert!(html.contains("&lt;U&gt;N&lt;/U&gt;"));
    assert!(html.contains("&lt;script&gt;v&lt;/script&gt;"));
    assert!(html.contains("&quot;&gt;&lt;script&gt;h&lt;/script&gt;"));
    assert!(html.contains("EMISOR &lt;b&gt;&amp;&lt;/b&gt;"));
    for raw in [
        "<script>alert",
        "<img src=x",
        "<script>v",
        "<script>h",
        "<u>n",
    ] {
        assert!(!html.contains(raw), "{raw} sin escapar");
    }
}

#[test]
fn template_names_cannot_leave_the_template_roots() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("plantillas");
    fs::create_dir(&inner).unwrap();
    fs::write(dir.path().join("secreto.html.twig"), "SECRETO").unwrap();
    for name in [
        "../secreto.html.twig",
        "assets/../../secreto.html.twig",
        "..",
        "a\0b",
    ] {
        let mut report = HtmlReport::new(ReportOptions::with_template_dir(&inner));
        report.set_template(name);
        let err = report.render_html(&invoice(), &params()).unwrap_err();
        assert!(
            matches!(err, ReportError::InvalidTemplateName { .. }),
            "{name}: {err:?}"
        );
    }
    // Una ruta absoluta se busca dentro de las raíces, como Twig.
    let mut report = HtmlReport::new(ReportOptions::with_template_dir(&inner));
    report.set_template(
        dir.path()
            .join("secreto.html.twig")
            .to_string_lossy()
            .into_owned(),
    );
    let err = report.render_html(&invoice(), &params()).unwrap_err();
    assert!(
        matches!(err, ReportError::TemplateNotFound { .. }),
        "{err:?}"
    );
}

#[cfg(unix)]
#[test]
fn symlinks_outside_the_user_dir_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("plantillas");
    fs::create_dir(&inner).unwrap();
    fs::write(dir.path().join("fuera.html.twig"), "FUERA").unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("fuera.html.twig"),
        inner.join("invoice.html.twig"),
    )
    .unwrap();
    fs::write(inner.join("dentro.html.twig"), "DENTRO {{ doc.serie }}").unwrap();
    std::os::unix::fs::symlink(
        inner.join("dentro.html.twig"),
        inner.join("alias.html.twig"),
    )
    .unwrap();
    let report = HtmlReport::new(ReportOptions::with_template_dir(&inner));
    let err = report.render_html(&invoice(), &params()).unwrap_err();
    assert!(
        matches!(err, ReportError::InvalidTemplateName { .. }),
        "{err:?}"
    );
    let mut report = HtmlReport::new(ReportOptions::with_template_dir(&inner));
    report.set_template("alias.html.twig");
    assert_eq!(
        report.render_html(&invoice(), &params()).unwrap(),
        "DENTRO F001"
    );
}

#[test]
fn user_template_size_and_encoding_are_limited() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("grande.html.twig"), vec![b'x'; 2048]).unwrap();
    fs::write(dir.path().join("latin1.html.twig"), b"\xf1and\xfa").unwrap();
    let mut options = ReportOptions::with_template_dir(dir.path());
    options.max_template_bytes = 1024;
    for name in ["grande.html.twig", "latin1.html.twig"] {
        let mut report = HtmlReport::new(options.clone());
        report.set_template(name);
        let err = report.render_html(&invoice(), &params()).unwrap_err();
        assert!(
            matches!(err, ReportError::TemplateIo { .. }),
            "{name}: {err:?}"
        );
    }
}

#[test]
fn logo_limit_is_enforced_and_configurable() {
    let mut p = params();
    p.system.logo = Some(vec![0u8; 2048]);
    let mut options = ReportOptions::default();
    options.max_logo_bytes = Some(1024);
    let err = HtmlReport::new(options.clone())
        .render_html(&invoice(), &p)
        .unwrap_err();
    assert!(
        matches!(
            err,
            ReportError::Render {
                cause: RenderCause::Limit(_),
                ..
            }
        ),
        "{err:?}"
    );
    options.max_logo_bytes = None;
    assert!(HtmlReport::new(options).render_html(&invoice(), &p).is_ok());
}

#[test]
fn fuel_and_recursion_stop_runaway_templates() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("bucle.html.twig"),
        "{% for i in range(100000) %}{{ i }}{% endfor %}",
    )
    .unwrap();
    fs::write(
        dir.path().join("propia.html.twig"),
        "{% include 'propia.html.twig' %}",
    )
    .unwrap();
    let mut options = ReportOptions::with_template_dir(dir.path());
    options.fuel = Some(10_000);
    let mut report = HtmlReport::new(options);
    report.set_template("bucle.html.twig");
    let err = report.render_html(&invoice(), &params()).unwrap_err();
    assert!(
        matches!(
            err,
            ReportError::Render {
                cause: RenderCause::Limit(_),
                ..
            }
        ),
        "{err:?}"
    );
    let mut report = HtmlReport::new(ReportOptions::with_template_dir(dir.path()));
    report.set_template("propia.html.twig");
    assert!(report.render_html(&invoice(), &params()).is_err());
}

#[test]
fn user_template_cannot_reach_files_or_environment() {
    // Sin funciones de archivo, red ni entorno en el motor: solo las registradas.
    let dir = tempfile::tempdir().unwrap();
    for (name, body) in [
        ("a.html.twig", "{{ include('/etc/passwd') }}"),
        ("b.html.twig", "{{ source('/etc/passwd') }}"),
        ("c.html.twig", "{{ env('HOME') }}"),
    ] {
        fs::write(dir.path().join(name), body).unwrap();
        let mut report = HtmlReport::new(ReportOptions::with_template_dir(dir.path()));
        report.set_template(name);
        assert!(report.render_html(&invoice(), &params()).is_err(), "{name}");
    }
}

#[test]
fn malicious_xml_is_rejected() {
    let utils = XmlUtils::default();
    let laughs = br#"<!DOCTYPE l [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">]><l>&b;</l>"#;
    assert_eq!(utils.hash_sign(laughs), Err(XmlUtilsError::Dtd));
    let deep = format!("{}{}", "<a>".repeat(100_000), "</a>".repeat(100_000));
    assert!(matches!(
        utils.hash_sign(deep.as_bytes()),
        Err(XmlUtilsError::Limit(_))
    ));
    let small = XmlUtils {
        max_bytes: 16,
        ..XmlUtils::default()
    };
    assert!(matches!(
        small.hash_sign(b"<a>xxxxxxxxxxxxxxxxxxxx</a>"),
        Err(XmlUtilsError::Limit(_))
    ));
    assert!(matches!(
        utils.hash_sign(b"\xff\xfe<a/>"),
        Err(XmlUtilsError::UnsupportedEncoding(_))
    ));
}

#[test]
fn qr_svg_contains_only_drawing_elements() {
    let svg = cpeforge_report::qr::svg("x\"><script>".as_bytes()).unwrap();
    assert!(!svg.contains("script"));
    assert!(svg.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg "));
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let names: Vec<_> = doc
        .descendants()
        .filter(|n| n.is_element())
        .map(|n| n.tag_name().name())
        .collect();
    assert_eq!(names, ["svg", "rect", "g", "g", "path"]);
}
