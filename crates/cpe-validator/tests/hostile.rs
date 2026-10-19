//! XML y XSL hostiles sin red: DTD/XXE, escapes de directorio, red,
//! escritura, agotamiento de recursos, funciones no registradas,
//! concurrencia y mutaciones acotadas (sin pánicos). Las URL de red apuntan a
//! 127.0.0.1:9 y la política las rechaza antes de conectar; el runner de
//! paridad ejecuta además estas pruebas sin red (sandbox/iptables).

use std::fs;
use std::path::{Path, PathBuf};

use cpeforge_cpe_validator::{
    AccessKind, CpeValidator, CpeValidatorError, InputSource, LimitKind, Limits, MATCHES_HANDLER,
    Stylesheet, Validator, XmlDocument, XslValidator, engine_info,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CBC: &str = "urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2";

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> TestResult<Self> {
        let path = std::env::temp_dir().join(format!(
            "cpeforge-cpe-hostile-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("rules/2.X"))?;
        fs::create_dir_all(path.join("outside"))?;
        Ok(Self(path))
    }
    fn rules(&self) -> PathBuf {
        self.0.join("rules")
    }
    fn outside(&self) -> PathBuf {
        self.0.join("outside")
    }
    fn write(&self, rel: &str, content: &str) -> TestResult<PathBuf> {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(path)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sheet(body: &str, extra: &str) -> String {
    format!(
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:cbc="{CBC}" xmlns:php="http://php.net/xsl"{extra}><xsl:param name="nombreArchivoEnviado"/>{body}</xsl:stylesheet>"#
    )
}

fn invoice() -> Result<XmlDocument, CpeValidatorError> {
    XmlDocument::parse(format!(
        "<Invoice xmlns:cbc=\"{CBC}\"><cbc:ID>F001-1</cbc:ID></Invoice>"
    ))
}

fn run(
    dir: &Dir,
    body: &str,
    extra: &str,
    limits: Limits,
) -> Result<Vec<cpeforge_cpe_validator::CpeError>, CpeValidatorError> {
    let path = dir
        .write("rules/main.xsl", &sheet(body, extra))
        .map_err(|_| CpeValidatorError::Engine("io"))?;
    let stylesheet = Stylesheet::from_file(&path, Some(&dir.rules()), &limits)?;
    let mut validator = XslValidator::default().with_limits(limits);
    validator.set_xsl(stylesheet);
    validator.validate("f.xml", &invoice()?)
}

fn assert_denied(result: Result<impl std::fmt::Debug, CpeValidatorError>, kind: AccessKind) {
    assert!(
        matches!(&result, Err(CpeValidatorError::AccessDenied { kind: got, .. }) if *got == kind),
        "se esperaba AccessDenied({kind:?}): {result:?}"
    );
}

#[test]
fn xml_with_doctype_is_rejected_before_any_entity() {
    for xml in [
        r#"<!DOCTYPE x [<!ENTITY e SYSTEM "file:///etc/passwd">]><x>&e;</x>"#,
        r#"<!DOCTYPE x SYSTEM "http://127.0.0.1:9/x.dtd"><x/>"#,
        r#"<!DOCTYPE x [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;"><!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;"><!ENTITY d "&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;"><!ENTITY e "&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;">]><x>&e;&e;&e;&e;</x>"#,
        r#"<!DOCTYPE x [<!ENTITY % p SYSTEM "http://127.0.0.1:9/p.dtd"> %p;]><x/>"#,
    ] {
        // libxml2 puede abortar antes por su límite de amplificación de
        // entidades (XML mal formado); en ningún caso se expande ni se lee.
        let result = XmlDocument::parse(xml);
        assert!(
            matches!(
                result,
                Err(CpeValidatorError::DoctypeForbidden {
                    source: InputSource::Xml
                } | CpeValidatorError::MalformedXml { .. })
            ),
            "{xml}: {result:?}"
        );
    }
}

#[test]
fn malformed_and_oversized_xml_fail_closed() {
    let deep = format!("{}{}", "<a>".repeat(10_000), "</a>".repeat(10_000));
    for xml in [
        deep.as_bytes(),
        b"\xff\xfe<\x00x".as_slice(),
        b"<a>\x00</a>",
        b"<a",
        b"texto",
    ] {
        assert!(
            matches!(
                XmlDocument::parse(xml),
                Err(CpeValidatorError::MalformedXml { .. })
            ),
            "{:?}",
            String::from_utf8_lossy(xml)
        );
    }
    assert!(matches!(
        XmlDocument::parse(""),
        Err(CpeValidatorError::EmptyXml)
    ));
    let limits = Limits::default().with_max_xml_bytes(64);
    let big = format!("<a>{}</a>", "x".repeat(100));
    assert!(matches!(
        XmlDocument::parse_with_limits(big, &limits),
        Err(CpeValidatorError::TooLarge {
            actual: 107,
            limit: 64,
            ..
        })
    ));
}

#[test]
fn stylesheet_doctype_and_includes_outside_root_are_rejected() -> TestResult {
    let dir = Dir::new("include")?;
    let passwd = dir.write("outside/secret.xml", "<s>secreto</s>")?;
    let limits = Limits::default();
    let doctype = dir.write(
        "rules/dtd.xsl",
        &format!(
            "<!DOCTYPE xsl:stylesheet SYSTEM \"http://127.0.0.1:9/x.dtd\">{}",
            sheet("", "")
        ),
    )?;
    assert!(matches!(
        Stylesheet::from_file(&doctype, Some(&dir.rules()), &limits),
        Err(CpeValidatorError::DoctypeForbidden { .. })
    ));
    for href in [
        format!("file://{}", passwd.display()),
        passwd.display().to_string(),
        "../outside/secret.xml".to_owned(),
        "http://127.0.0.1:9/inc.xsl".to_owned(),
        "ftp://127.0.0.1:9/inc.xsl".to_owned(),
    ] {
        let path = dir.write(
            "rules/inc.xsl",
            &sheet(&format!("<xsl:include href=\"{href}\"/>"), ""),
        )?;
        let result = Stylesheet::from_file(&path, Some(&dir.rules()), &limits);
        let network = href.starts_with("http") || href.starts_with("ftp");
        assert_denied(
            result,
            if network {
                AccessKind::ReadNetwork
            } else {
                AccessKind::ReadFile
            },
        );
    }
    // Sin directorio autorizado (hoja en memoria) no se lee ningún archivo.
    let inline = sheet(&format!("<xsl:include href=\"{}\"/>", passwd.display()), "");
    assert_denied(
        Stylesheet::from_bytes(inline, None, &limits),
        AccessKind::ReadFile,
    );
    // La hoja principal fuera del directorio o por enlace simbólico.
    let outside = dir.write("outside/regla.xsl", &sheet("", ""))?;
    assert!(matches!(
        Stylesheet::from_file(&outside, Some(&dir.rules()), &limits),
        Err(CpeValidatorError::RuleOutsideRoot { .. })
    ));
    let link = dir.rules().join("2.X/ValidaExprRegFactura-2.0.1.xsl");
    std::os::unix::fs::symlink(&outside, &link)?;
    let validator = CpeValidator::new(dir.rules());
    let result = validator.validate_xml("f.xml", "<Invoice/>");
    assert!(
        matches!(result, Err(CpeValidatorError::RuleOutsideRoot { .. })),
        "{result:?}"
    );
    Ok(())
}

#[test]
fn runtime_document_network_and_writes_are_rejected() -> TestResult {
    let dir = Dir::new("runtime")?;
    let secret = dir.write("outside/secret.xml", "<s>secreto</s>")?;
    fs::create_dir_all(dir.rules().join("subdir"))?;
    std::os::unix::fs::symlink(&secret, dir.rules().join("enlace.xml"))?;
    dir.write("rules/grande.xml", &format!("<g>{}</g>", "x".repeat(4096)))?;
    let limits = Limits::default().with_max_stylesheet_bytes(2048);
    let msg = |select: &str| {
        format!(
            r#"<xsl:template match="/"><xsl:message>1|D|<xsl:value-of select="{select}"/></xsl:message></xsl:template>"#
        )
    };
    for (select, kind) in [
        (
            format!("count(document('{}'))", secret.display()),
            AccessKind::ReadFile,
        ),
        (
            "count(document('enlace.xml'))".to_owned(),
            AccessKind::ReadFile,
        ),
        ("count(document('subdir'))".to_owned(), AccessKind::ReadFile),
        (
            "count(document('grande.xml'))".to_owned(),
            AccessKind::ReadFile,
        ),
        (
            "count(document('http://127.0.0.1:9/x.xml'))".to_owned(),
            AccessKind::ReadNetwork,
        ),
    ] {
        assert_denied(run(&dir, &msg(&select), "", limits), kind);
    }
    // dyn:evaluate no evita la política.
    let dynamic = format!(
        r#"<xsl:variable name="e">count(document('{}'))</xsl:variable>{}"#,
        secret.display(),
        msg("dyn:evaluate(string($e))")
    );
    assert_denied(
        run(
            &dir,
            &dynamic,
            " xmlns:dyn=\"http://exslt.org/dynamic\"",
            limits,
        ),
        AccessKind::ReadFile,
    );
    let target = dir.outside().join("escrito.xml");
    let write = format!(
        r#"<xsl:template match="/"><exsl:document href="{}"><x/></exsl:document></xsl:template>"#,
        target.display()
    );
    assert_denied(
        run(
            &dir,
            &write,
            " xmlns:exsl=\"http://exslt.org/common\" extension-element-prefixes=\"exsl\"",
            limits,
        ),
        AccessKind::WriteFile,
    );
    assert!(!target.exists());
    Ok(())
}

#[test]
fn resource_limits_are_typed_errors() -> TestResult {
    let dir = Dir::new("limits")?;
    let limits = Limits::default();
    let recursion = r#"<xsl:template match="/"><xsl:call-template name="r"/></xsl:template><xsl:template name="r"><xsl:call-template name="r"/></xsl:template>"#;
    assert!(matches!(
        run(&dir, recursion, "", limits),
        Err(CpeValidatorError::ResourceLimit {
            kind: LimitKind::TemplateDepth
        })
    ));
    let regex = format!(
        r#"<xsl:template match="/"><xsl:value-of select="php:function('{MATCHES_HANDLER}', '^(a+)+$', '{}b')"/></xsl:template>"#,
        "a".repeat(40)
    );
    assert!(matches!(
        run(&dir, &regex, "", limits),
        Err(CpeValidatorError::ResourceLimit {
            kind: LimitKind::Regex
        })
    ));
    // Coste XPath exponencial con un límite pequeño.
    let costly = r#"<xsl:template match="/"><xsl:message>1|C|<xsl:value-of select="count(//*[count(//*[count(//*) &gt; 0]) &gt; 0])"/></xsl:message></xsl:template>"#;
    let wide = XmlDocument::parse(format!("<r>{}</r>", "<a><b/><b/><b/></a>".repeat(30)))?;
    let path = dir.write("rules/costly.xsl", &sheet(costly, ""))?;
    let validate =
        |limits: Limits| -> Result<Vec<cpeforge_cpe_validator::CpeError>, CpeValidatorError> {
            let mut validator = XslValidator::default().with_limits(limits);
            validator.set_xsl(Stylesheet::from_file(&path, Some(&dir.rules()), &limits)?);
            validator.validate("f.xml", &wide)
        };
    let result = validate(Limits::default().with_xpath_operations(100_000));
    // Linux y binarios macOS con objetivo >= 13 deben aplicar el límite; el
    // runner de paridad lo exige con CPE_EXPECT_OP_LIMIT=1.
    if std::env::var("CPE_EXPECT_OP_LIMIT").as_deref() == Ok("1") {
        assert!(engine_info().xpath_operation_limit, "{:?}", engine_info());
    }
    if engine_info().xpath_operation_limit {
        assert!(
            matches!(
                result,
                Err(CpeValidatorError::ResourceLimit {
                    kind: LimitKind::XPathOperations
                })
            ),
            "{result:?}"
        );
        // Con el límite por defecto la misma hoja termina.
        assert_eq!(validate(Limits::default())?.len(), 1);
    } else {
        assert!(result.is_ok(), "{result:?}");
    }
    Ok(())
}

#[test]
fn only_matches_is_callable() -> TestResult {
    let dir = Dir::new("functions")?;
    for name in [
        "system",
        "exec",
        "file_get_contents",
        "CPEForge\\Validator\\Xml\\XsltFunctions::other",
        "XsltFunctions::matches",
    ] {
        let body = format!(
            r#"<xsl:template match="/"><xsl:value-of select="php:function('{name}', 'id')"/></xsl:template>"#
        );
        let result = run(&dir, &body, "", Limits::default());
        assert!(
            matches!(&result, Err(CpeValidatorError::Extension { class: "Error", message }) if message.contains(name)),
            "{name}: {result:?}"
        );
    }
    Ok(())
}

#[test]
fn rules_root_with_spaces_and_relative_includes() -> TestResult {
    let dir = Dir::new("espacios")?;
    // El espacio y la «ñ» están en el directorio base (URL escapada con
    // xmlPathToURI, como la referencia); los href relativos son URI válidas.
    let root = dir.0.join("reglas con espacio ñ");
    fs::create_dir_all(root.join("2.X"))?;
    fs::create_dir_all(root.join("comun"))?;
    fs::write(
        root.join("comun/inc.xsl"),
        r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform"><xsl:template name="t"><xsl:message>1|INC|<xsl:value-of select="count(document('datos.xml')/d)"/></xsl:message></xsl:template></xsl:stylesheet>"#,
    )?;
    fs::write(root.join("comun/datos.xml"), "<d/>")?;
    fs::write(
        root.join("2.X/ValidaExprRegFactura-2.0.1.xsl"),
        sheet(
            r#"<xsl:include href="../comun/inc.xsl"/><xsl:template match="/"><xsl:call-template name="t"/></xsl:template>"#,
            "",
        ),
    )?;
    let errors = CpeValidator::new(&root).validate_xml("f.xml", "<Invoice/>")?;
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message.as_deref(), Some("1"));
    Ok(())
}

#[test]
fn concurrent_validations_do_not_mix_messages() -> TestResult {
    let dir = Dir::new("threads")?;
    let path = dir.write(
        "rules/2.X/ValidaExprRegFactura-2.0.1.xsl",
        &sheet(r#"<xsl:template match="/*"><xsl:message>1|T|<xsl:value-of select="$nombreArchivoEnviado"/>|<xsl:value-of select="cbc:ID"/>|x</xsl:message></xsl:template>"#, ""),
    )?;
    let _ = path;
    let validator = std::sync::Arc::new(CpeValidator::new(dir.rules()));
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let validator = std::sync::Arc::clone(&validator);
            std::thread::spawn(move || -> Result<(), String> {
                for i in 0..25 {
                    let name = format!("hilo-{t}-{i}.xml");
                    let xml =
                        format!("<Invoice xmlns:cbc=\"{CBC}\"><cbc:ID>F{t}-{i}</cbc:ID></Invoice>");
                    let errors = validator
                        .validate_xml(&name, xml)
                        .map_err(|e| e.to_string())?;
                    let got = errors.first().and_then(|e| e.message.clone());
                    if errors.len() != 1 || got.as_deref() != Some(name.as_str()) {
                        return Err(format!("{name}: {errors:?}"));
                    }
                    if errors[0].node_path.as_deref() != Some(format!("F{t}-{i}").as_str()) {
                        return Err(format!("{name}: {errors:?}"));
                    }
                }
                Ok(())
            })
        })
        .collect();
    for handle in handles {
        handle.join().map_err(|_| "hilo con pánico")??;
    }
    Ok(())
}

/// Generador determinista (xorshift) para mutaciones reproducibles.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
    }
}

fn mutate(rng: &mut Rng, seed: &[u8]) -> Vec<u8> {
    let mut out = seed.to_vec();
    for _ in 0..=rng.below(4) {
        let tokens: [&[u8]; 10] = [
            b"<",
            b">",
            b"&",
            b"|",
            b"\x00",
            b"<!DOCTYPE x>",
            b"]]>",
            b"\xff",
            b"'",
            b"/",
        ];
        let pos = rng.below(out.len() + 1);
        match rng.below(4) {
            0 if !out.is_empty() => {
                let at = pos.min(out.len() - 1);
                out[at] = u8::try_from(rng.next() & 0xff).unwrap_or(0);
            }
            1 => out.truncate(pos),
            2 => {
                let token = tokens[rng.below(tokens.len())];
                out.splice(pos..pos, token.iter().copied());
            }
            _ => {
                let len = rng.below(16);
                let end = (pos + len).min(out.len());
                out.drain(pos..end);
            }
        }
    }
    out
}

#[test]
fn bounded_mutation_fuzzing_never_panics() -> TestResult {
    let dir = Dir::new("fuzz")?;
    let xml_seed = format!(
        "<Invoice xmlns:cbc=\"{CBC}\"><cbc:UBLVersionID>2.1</cbc:UBLVersionID><cbc:ID>F001-1</cbc:ID></Invoice>"
    );
    let xsl_seed = sheet(
        &format!(
            r#"<xsl:template match="/*"><xsl:if test="not(php:function('{MATCHES_HANDLER}', '^(2.1)$', string(cbc:UBLVersionID)) = 1)"><xsl:message terminate="yes">1|2074|v|p|<xsl:value-of select="cbc:UBLVersionID"/></xsl:message></xsl:if></xsl:template>"#
        ),
        "",
    );
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let base = Stylesheet::from_bytes(&xsl_seed, None, &Limits::default())?;
    let mut validator = XslValidator::default();
    validator.set_xsl(base);
    let mut outcomes = [0usize; 2];
    for _ in 0..400 {
        let xml = mutate(&mut rng, xml_seed.as_bytes());
        if let Ok(document) = XmlDocument::parse(&xml) {
            outcomes[usize::from(validator.validate("f.xml", &document).is_ok())] += 1;
        }
    }
    for i in 0..300 {
        let xsl = mutate(&mut rng, xsl_seed.as_bytes());
        let path = dir.write("rules/fuzz.xsl", &String::from_utf8_lossy(&xsl))?;
        let from_file = Stylesheet::from_file(&path, Some(&dir.rules()), &Limits::default());
        let from_bytes = Stylesheet::from_bytes(&xsl, None, &Limits::default());
        for sheet in [from_file, from_bytes].into_iter().flatten() {
            let mut v = XslValidator::default();
            v.set_xsl(sheet);
            outcomes[usize::from(v.validate(&format!("f{i}.xml"), &invoice()?).is_ok())] += 1;
        }
    }
    // El corpus ejercita tanto éxitos como errores tipados.
    assert!(outcomes[0] > 0 && outcomes[1] > 0, "{outcomes:?}");
    Ok(())
}

#[test]
fn no_file_was_created_outside_temp() {
    // Ninguna prueba escribe junto al crate.
    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("escrito.xml")
            .exists()
    );
}
