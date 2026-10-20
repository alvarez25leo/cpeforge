//! Compila `csrc/glue.c` contra las cabeceras de libxml2/libxslt/libexslt del
//! sistema y enlaza esas bibliotecas (las mismas que usa `ext-xsl` de la referencia).
//!
//! - Linux y otros Unix: `pkg-config` (`libxslt`, `libexslt`; paquetes
//!   `libxslt1-dev`/`libxml2-dev` en Debian/Ubuntu).
//! - macOS: las bibliotecas del SDK (`xcrun --show-sdk-path`).
//!
//! Windows no está soportado: no hay una distribución de libxslt estándar.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=csrc/glue.c");
    println!("cargo:rerun-if-env-changed=SDKROOT");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mut build = cc::Build::new();
    build
        .file("csrc/glue.c")
        .warnings(true)
        .flag_if_supported("-Wno-deprecated-declarations")
        .flag_if_supported("-Wno-unused-parameter");

    match target_os.as_str() {
        "windows" => {
            println!(
                "cargo:warning=cpeforge-cpe-validator requiere libxslt; Windows no está soportado"
            );
            std::process::exit(1);
        }
        "macos" => {
            let sdk = env::var("SDKROOT")
                .ok()
                .map(PathBuf::from)
                .or_else(macos_sdk);
            let Some(sdk) = sdk else {
                println!("cargo:warning=no se encontró el SDK de macOS (xcrun --show-sdk-path)");
                std::process::exit(1);
            };
            build
                .include(sdk.join("usr/include/libxml2"))
                .include(sdk.join("usr/include"));
            // Las cabeceras de Apple solo declaran opLimit/opCount de
            // xmlXPathContext (presentes en libxml2 >= 2.9.11, que el sistema
            // trae desde macOS 13) si el objetivo mínimo es >= 13. glue.c
            // comprueba además la versión en tiempo de ejecución antes de
            // usarlos; se fija la macro solo en esta unidad de compilación
            // para no cambiar el objetivo del binario.
            build
                .flag("-U__MAC_OS_X_VERSION_MIN_REQUIRED")
                .flag("-D__MAC_OS_X_VERSION_MIN_REQUIRED=130000")
                .flag_if_supported("-Wno-macro-redefined")
                .flag_if_supported("-Wno-builtin-macro-redefined");
            for lib in ["exslt", "xslt", "xml2"] {
                println!("cargo:rustc-link-lib=dylib={lib}");
            }
        }
        _ => {
            // Mínimos también comprobados en tiempo de ejecución (src/ffi.rs).
            for (lib, minimum) in [
                ("libexslt", None),
                ("libxslt", Some("1.1.34")),
                ("libxml-2.0", Some("2.9.11")),
            ] {
                let mut config = pkg_config::Config::new();
                if let Some(minimum) = minimum {
                    config.atleast_version(minimum);
                }
                match config.probe(lib) {
                    Ok(found) => {
                        for path in found.include_paths {
                            build.include(path);
                        }
                    }
                    Err(error) => {
                        println!("cargo:warning=no se encontró {lib} con pkg-config: {error}");
                        std::process::exit(1);
                    }
                }
            }
        }
    }
    build.compile("cpeforge_cpe_glue");
}

fn macos_sdk() -> Option<PathBuf> {
    let output = Command::new("xcrun").arg("--show-sdk-path").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    Some(PathBuf::from(path.trim()))
}
