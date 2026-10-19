//! Lee imágenes PGM (P5) y escribe el contenido QR decodificado por `rqrr`.
//!
//! Uso: report-qr-verify <imagen.pgm>...
//! Salida: una línea JSON por imagen: {"file": …, "content": …} o {"file": …, "error": …}.

use std::fs;

fn parse_pgm(bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let mut fields = Vec::new();
    let mut pos = 0;
    while fields.len() < 4 {
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let start = pos;
        while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        fields.push(String::from_utf8_lossy(&bytes[start..pos]).into_owned());
    }
    pos += 1;
    if fields[0] != "P5" {
        return Err("no es PGM P5".into());
    }
    let w: usize = fields[1].parse().map_err(|_| "ancho")?;
    let h: usize = fields[2].parse().map_err(|_| "alto")?;
    let data = bytes.get(pos..pos + w * h).ok_or("datos incompletos")?.to_vec();
    Ok((w, h, data))
}

fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn main() {
    for file in std::env::args().skip(1) {
        let result = fs::read(&file)
            .map_err(|e| e.to_string())
            .and_then(|bytes| parse_pgm(&bytes))
            .and_then(|(w, h, data)| {
                let mut img = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| data[y * w + x]);
                let grids = img.detect_grids();
                let grid = grids.first().ok_or("sin código QR")?;
                let (_, content) = grid.decode().map_err(|e| format!("{e:?}"))?;
                Ok(content)
            });
        match result {
            Ok(content) => println!("{{\"file\":\"{}\",\"content\":\"{}\"}}", escape(&file), escape(&content)),
            Err(error) => println!("{{\"file\":\"{}\",\"error\":\"{}\"}}", escape(&file), escape(&error)),
        }
    }
}
