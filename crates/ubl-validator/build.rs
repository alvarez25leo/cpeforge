//! Enlaza libxml2 del sistema para el motor XSD 1.0.

use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=csrc/schema.c");
    let mut build = cc::Build::new();
    build.file("csrc/schema.c").warnings(true);
    match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => {
            let sdk = env::var("SDKROOT").ok().map(PathBuf::from).or_else(|| {
                let output = Command::new("xcrun").arg("--show-sdk-path").output().ok()?;
                output
                    .status
                    .success()
                    .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
            });
            let Some(sdk) = sdk else {
                eprintln!("macOS SDK requerido para libxml2");
                std::process::exit(1);
            };
            build.include(sdk.join("usr/include/libxml2"));
            println!("cargo:rustc-link-lib=dylib=xml2");
        }
        Ok("windows") => {
            eprintln!("libxml2 del sistema no está configurada en Windows");
            std::process::exit(1);
        }
        _ => {
            // Mínimo también comprobado en tiempo de ejecución (src/ffi.rs).
            let found = match pkg_config::Config::new()
                .atleast_version("2.9.11")
                .probe("libxml-2.0")
            {
                Ok(found) => found,
                Err(error) => {
                    eprintln!("libxml2-dev requerido: {error}");
                    std::process::exit(1);
                }
            };
            for path in found.include_paths {
                build.include(path);
            }
        }
    }
    build.compile("cpeforge_ubl_schema");
}
