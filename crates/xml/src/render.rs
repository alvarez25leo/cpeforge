//! Escritor de las plantillas: escape, números, fechas y `spaceless`.

use cpeforge_core::time::{peru_time_zone, to_peru_time};
use cpeforge_core::{CoreError, Decimal, Zoned};

use crate::error::XmlError;
use std::fmt::Write as _;

use crate::format::{write_n_format, write_xsd_float, write_xsd_n_format_limit};
use crate::options::{Autoescape, RenderOptions};

pub(crate) type Result<T> = std::result::Result<T, XmlError>;

/// Contexto de lectura de atributos: plantilla y modo estricto.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ctx {
    strict_variables: bool,
    template: &'static str,
}

impl Ctx {
    /// Lee un atributo de un objeto que puede ser nulo (`objeto.atributo`).
    ///
    /// Con `strict_variables` un objeto nulo es un error; si no, el valor es nulo.
    pub(crate) fn get<'a, P: ?Sized, T>(
        self,
        parent: Option<&'a P>,
        attribute: &'static str,
        read: impl FnOnce(&'a P) -> Option<T>,
    ) -> Result<Option<T>> {
        match parent {
            Some(parent) => Ok(read(parent)),
            None if self.strict_variables => Err(XmlError::NullAttribute {
                template: self.template,
                attribute,
            }),
            None => Ok(None),
        }
    }

    /// Como [`Ctx::get`] para un objeto anidado (`objeto.hijo`).
    pub(crate) fn obj<'a, P: ?Sized, T: ?Sized>(
        self,
        parent: Option<&'a P>,
        attribute: &'static str,
        read: impl FnOnce(&'a P) -> Option<&'a T>,
    ) -> Result<Option<&'a T>> {
        self.get(parent, attribute, read)
    }

    /// Error de un método del modelo durante el render.
    pub(crate) fn document(self, source: CoreError) -> XmlError {
        XmlError::Document {
            template: self.template,
            source,
        }
    }
}

/// Salida de una plantilla en construcción.
pub(crate) struct Out<'o> {
    buf: String,
    options: &'o RenderOptions,
    ctx: Ctx,
}

impl<'o> Out<'o> {
    pub(crate) fn new(options: &'o RenderOptions, template: &'static str) -> Self {
        let ctx = Ctx {
            strict_variables: options.strict_variables,
            template,
        };
        Self {
            buf: String::with_capacity(8 * 1024),
            options,
            ctx,
        }
    }

    /// Contexto de lectura de atributos de esta plantilla.
    pub(crate) fn ctx(&self) -> Ctx {
        self.ctx
    }

    /// Texto literal de la plantilla.
    pub(crate) fn raw(&mut self, markup: &str) -> &mut Self {
        self.buf.push_str(markup);
        self
    }

    /// `{{ texto }}`: escapado según `autoescape`.
    pub(crate) fn text(&mut self, value: Option<&str>) -> &mut Self {
        if let Some(value) = value {
            match self.options.autoescape {
                Autoescape::Html => escape_html(&mut self.buf, value),
                Autoescape::Disabled => self.buf.push_str(value),
            }
        }
        self
    }

    /// `<![CDATA[{{ texto|raw }}]]>`: contenido de una sección CDATA, sin
    /// escape. Solo se usa entre `<![CDATA[` y `]]>` de la plantilla.
    ///
    /// Un `]]>` dentro del valor cerraría la sección antes de tiempo y
    /// permitiría insertar elementos en el comprobante (XML 1.0 §2.7). Se
    /// divide en dos secciones contiguas, `]]]]><![CDATA[>`, que el
    /// analizador XML lee como el mismo texto. Sin `]]>` la salida es la de
    /// la plantilla original.
    pub(crate) fn cdata(&mut self, value: Option<&str>) -> &mut Self {
        if let Some(value) = value {
            let mut rest = value;
            while let Some(pos) = rest.find("]]>") {
                self.buf.push_str(&rest[..pos]);
                self.buf.push_str("]]]]><![CDATA[>");
                rest = &rest[pos + 3..];
            }
            self.buf.push_str(rest);
        }
        self
    }

    /// `{{ numero }}` con un `float`: conversión de la referencia con
    /// `precision=14`, sin notación exponencial (`xsd:decimal` no la admite;
    /// ver [`crate::format::xsd_float_string`]).
    pub(crate) fn num(&mut self, value: Option<Decimal>) -> &mut Self {
        if let Some(value) = value {
            write_xsd_float(&mut self.buf, value);
        }
        self
    }

    /// `{{ entero }}`.
    pub(crate) fn int(&mut self, value: Option<i64>) -> &mut Self {
        if let Some(value) = value {
            let _ = write!(self.buf, "{value}");
        }
        self
    }

    /// `{{ loop.index }}`.
    pub(crate) fn index(&mut self, index: usize) -> &mut Self {
        let _ = write!(self.buf, "{}", index + 1);
        self
    }

    /// `{{ x|n_format(n) }}`.
    pub(crate) fn fmt(&mut self, value: Option<Decimal>, decimals: u32) -> &mut Self {
        write_n_format(&mut self.buf, value, decimals);
        self
    }

    /// `{{ x|n_format }}` (dos decimales).
    pub(crate) fn amount(&mut self, value: Option<Decimal>) -> &mut Self {
        self.fmt(value, 2)
    }

    /// `{{ x|n_format_limit(n) }}`, sin ceros de relleno donde la referencia
    /// usaría exponente (ver [`crate::format::xsd_n_format_limit`]).
    pub(crate) fn limit(&mut self, value: Option<Decimal>, decimals: u32) -> &mut Self {
        write_xsd_n_format_limit(&mut self.buf, value, decimals);
        self
    }

    /// `{{ fecha|date('Y-m-d') }}` en hora de Perú; con `null`, la fecha actual.
    pub(crate) fn date(&mut self, value: Option<&Zoned>) -> Result<&mut Self> {
        let local = self.local(value)?;
        let year = i32::from(local.year());
        let sign = if year < 0 { "-" } else { "" };
        let _ = write!(
            self.buf,
            "{sign}{:04}-{:02}-{:02}",
            year.unsigned_abs(),
            local.month(),
            local.day()
        );
        Ok(self)
    }

    /// `{{ fecha|date('H:i:s') }}` en hora de Perú; con `null`, la hora actual.
    pub(crate) fn time(&mut self, value: Option<&Zoned>) -> Result<&mut Self> {
        let local = self.local(value)?;
        let _ = write!(
            self.buf,
            "{:02}:{:02}:{:02}",
            local.hour(),
            local.minute(),
            local.second()
        );
        Ok(self)
    }

    fn local(&self, value: Option<&Zoned>) -> Result<jiff::civil::DateTime> {
        let zoned = match value {
            Some(date) => to_peru_time(date),
            None => peru_time_zone().map(|tz| self.options.current_time().to_zoned(tz)),
        };
        zoned
            .map(|z| z.datetime())
            .map_err(|source| self.ctx.document(source))
    }

    /// Resultado final tras `{% apply spaceless %}`.
    pub(crate) fn finish(self) -> String {
        spaceless_in_place(self.buf)
    }
}

/// `htmlspecialchars($s, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8')`. Copia por
/// tramos entre los caracteres escapados (todos ASCII, así que los cortes
/// caen en límites de carácter).
fn escape_html(out: &mut String, value: &str) {
    let mut start = 0;
    for (index, byte) in value.bytes().enumerate() {
        let replacement = match byte {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#039;",
            _ => continue,
        };
        out.push_str(&value[start..index]);
        out.push_str(replacement);
        start = index + 1;
    }
    out.push_str(&value[start..]);
}

const fn is_pcre_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

const fn is_trim_char(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0 | 0x0B)
}

/// [`spaceless`] sobre el propio buffer, sin una segunda copia del XML: solo
/// se eliminan bytes ASCII de espacio situados entre `>` y `<`, así que el
/// resultado sigue siendo UTF-8 válido.
fn spaceless_in_place(text: String) -> String {
    let mut bytes = text.into_bytes();
    let len = bytes.len();
    // Mismo recorrido que [`spaceless`]; los tramos conservados se mueven por
    // bloques hacia delante dentro del buffer.
    let (mut write, mut copied, mut index) = (0, 0, 0);
    while let Some(offset) = bytes[index..].iter().position(|b| *b == b'>') {
        index += offset;
        let mut end = index + 1;
        while end < len && is_pcre_space(bytes[end]) {
            end += 1;
        }
        if end > index + 1 && end < len && bytes[end] == b'<' {
            if write != copied {
                bytes.copy_within(copied..=index, write);
            }
            write += index + 1 - copied;
            copied = end;
        }
        index = end;
        if index >= len {
            break;
        }
    }
    if write != copied {
        bytes.copy_within(copied..len, write);
    }
    write += len - copied;
    bytes.truncate(write);
    let end = bytes
        .iter()
        .rposition(|b| !is_trim_char(*b))
        .map_or(0, |i| i + 1);
    bytes.truncate(end);
    let start = bytes.iter().position(|b| !is_trim_char(*b)).unwrap_or(end);
    if start > 0 {
        bytes.drain(..start);
    }
    // El buffer creció por duplicación: si sobra más de la mitad, se ajusta
    // para que quien conserve el XML no retenga hasta el doble de memoria.
    if bytes.capacity() > bytes.len() + bytes.len() / 2 {
        bytes.shrink_to_fit();
    }
    String::from_utf8(bytes)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

#[cfg(test)]
pub(crate) fn spaceless(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut copied = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'>' {
            let mut end = index + 1;
            while end < bytes.len()
                && matches!(bytes[end], b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
            {
                end += 1;
            }
            if end > index + 1 && end < bytes.len() && bytes[end] == b'<' {
                out.push_str(&input[copied..=index]);
                copied = end;
            }
            index = end;
        } else {
            index += 1;
        }
    }
    out.push_str(&input[copied..]);
    out.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\x0B'))
        .to_owned()
}

/// Verdad de Twig para cadenas: `""` y `"0"` son falsas.
pub(crate) fn truthy_str(value: Option<&str>) -> bool {
    matches!(value, Some(text) if !text.is_empty() && text != "0")
}

/// Verdad de Twig para `float`: `0.0` y `-0.0` son falsos.
pub(crate) fn truthy_num(value: Option<Decimal>) -> bool {
    value.is_some_and(|v| !v.is_zero())
}

/// Verdad de Twig para listas: un array vacío es falso.
pub(crate) fn truthy_list<T>(value: Option<&Vec<T>>) -> bool {
    value.is_some_and(|list| !list.is_empty())
}

/// Filtro `default`: usa `fallback` si el valor es nulo o una cadena vacía.
pub(crate) fn default_str<'a>(
    value: Option<&'a str>,
    fallback: Option<&'a str>,
) -> Option<&'a str> {
    match value {
        Some(text) if !text.is_empty() => Some(text),
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaceless_matches_twig() {
        assert_eq!(
            spaceless("  <a>  <b> x </b>\n\t<c/>  "),
            "<a><b> x </b><c/>"
        );
        assert_eq!(spaceless("<a>  </a>"), "<a></a>");
        assert_eq!(spaceless("<![CDATA[x>  <y]]>"), "<![CDATA[x><y]]>");
        assert_eq!(spaceless("<a>\u{a0}</a>"), "<a>\u{a0}</a>");
        assert_eq!(spaceless("a >"), "a >");
    }

    #[test]
    fn in_place_spaceless_matches_reference() {
        for input in [
            "  <a>  <b> x </b>\n\t<c/>  ",
            "<a>  </a>",
            "<![CDATA[x>  <y]]>",
            "<a>\u{a0}</a>",
            "a >",
            "> >  <",
            ">\u{c}\u{b}<",
            "",
            "   ",
            "\0<a/>\0",
            "<a>é  <b>ñ</b>  </a>  ",
            ">\t",
        ] {
            assert_eq!(
                spaceless_in_place(input.to_owned()),
                spaceless(input),
                "{input:?}"
            );
        }
    }

    #[test]
    fn cdata_splits_section_terminator() {
        let options = RenderOptions::default();
        let mut out = Out::new(&options, "t");
        out.raw("<a><![CDATA[")
            .cdata(Some("x]]><b>y</b>]]>"))
            .raw("]]></a>");
        assert_eq!(
            out.finish(),
            "<a><![CDATA[x]]]]><![CDATA[><b>y</b>]]]]><![CDATA[>]]></a>"
        );
    }

    #[test]
    fn escapes_html_by_runs() {
        let mut out = String::new();
        escape_html(&mut out, "a&b<c>d\"e'f ñ");
        assert_eq!(out, "a&amp;b&lt;c&gt;d&quot;e&#039;f ñ");
    }

    #[test]
    fn truthiness() {
        assert!(!truthy_str(Some("0")));
        assert!(truthy_str(Some("00")));
        assert!(!truthy_num(Some(Decimal::ZERO)));
        assert_eq!(default_str(Some(""), Some("PEN")), Some("PEN"));
        assert_eq!(default_str(Some("0"), Some("PEN")), Some("0"));
    }
}
