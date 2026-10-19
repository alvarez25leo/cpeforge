//! Configuración de `See` y `Api`, sin credenciales en el código: se leen de
//! `example/credenciales/credenciales-prueba.json` (o del JSON en
//! `EJEMPLO_CREDENCIALES`). El certificado es el que indica el campo
//! `certificado` del JSON, relativo a ese archivo, o el de `EJEMPLO_CERT`.
//!
//! Los servicios quedan fijos en los entornos de prueba (SUNAT BETA y la API
//! GRE de `gre.api_url`): este ejemplo nunca usa producción.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use cpeforge::{Api, See};
use cpeforge_gre_api::{Configuration, ReqwestTransport};
use cpeforge_ws::api::{ApiFactory, InMemoryStore, SystemClock};
use cpeforge_ws::soap::SunatEndpoints;
use serde_json::Value;

/// `Api` GRE con transporte real, almacén de token en memoria y reloj del sistema.
pub type ApiGre = Api<ReqwestTransport, InMemoryStore, SystemClock>;

/// Clave SOL de SUNAT BETA (`sunat_beta` del JSON).
struct ClaveSol {
    ruc: String,
    usuario: String,
    clave: String,
}

/// Credenciales de la API GRE de prueba (`gre` del JSON).
struct CredencialesGre {
    api_url: String,
    client_id: String,
    client_secret: String,
    ruc: String,
    usuario: String,
    clave: String,
}

/// Certificado y credenciales leídos del JSON de credenciales.
pub struct Config {
    pem: String,
    sol: ClaveSol,
    gre: Option<CredencialesGre>,
    transport: ReqwestTransport,
}

/// Texto obligatorio `objeto.campo` del JSON.
fn texto(json: &Value, objeto: &str, campo: &str, path: &Path) -> Result<String, Box<dyn Error>> {
    json[objeto][campo]
        .as_str()
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("Falta «{objeto}.{campo}» en {}", path.display()).into())
}

fn leer_gre(json: &Value, path: &Path) -> Result<Option<CredencialesGre>, Box<dyn Error>> {
    if json["gre"].is_null() {
        return Ok(None);
    }
    Ok(Some(CredencialesGre {
        api_url: texto(json, "gre", "api_url", path)?,
        client_id: texto(json, "gre", "client_id", path)?,
        client_secret: texto(json, "gre", "client_secret", path)?,
        ruc: texto(json, "gre", "ruc", path)?,
        usuario: texto(json, "gre", "usuario", path)?,
        clave: texto(json, "gre", "clave", path)?,
    }))
}

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

impl Config {
    /// Lee el JSON de credenciales y el certificado que indica.
    ///
    /// # Errors
    ///
    /// Archivos ilegibles, JSON inválido o incompleto, o fallo del transporte TLS.
    pub fn cargar() -> Result<Self, Box<dyn Error>> {
        let cred_path = std::env::var_os("EJEMPLO_CREDENCIALES").map_or_else(
            || raiz().join("example/credenciales/credenciales-prueba.json"),
            PathBuf::from,
        );
        let texto_json = fs::read_to_string(&cred_path).map_err(|e| {
            format!(
                "No se pudo leer el archivo de credenciales {}: {e}",
                cred_path.display()
            )
        })?;
        let json: Value = serde_json::from_str(&texto_json)
            .map_err(|e| format!("JSON inválido en {}: {e}", cred_path.display()))?;

        let cert_path = match std::env::var_os("EJEMPLO_CERT") {
            Some(path) => PathBuf::from(path),
            None => {
                let nombre = json["certificado"]
                    .as_str()
                    .ok_or_else(|| format!("Falta «certificado» en {}", cred_path.display()))?;
                cred_path.parent().unwrap_or(Path::new(".")).join(nombre)
            }
        };
        let pem = fs::read_to_string(&cert_path).map_err(|e| {
            format!(
                "No se pudo leer el certificado {}: {e}",
                cert_path.display()
            )
        })?;
        Ok(Self {
            pem,
            sol: ClaveSol {
                ruc: texto(&json, "sunat_beta", "ruc", &cred_path)?,
                usuario: texto(&json, "sunat_beta", "usuario", &cred_path)?,
                clave: texto(&json, "sunat_beta", "clave", &cred_path)?,
            },
            gre: leer_gre(&json, &cred_path)?,
            transport: ReqwestTransport::new()?,
        })
    }

    /// RUC de la Clave SOL.
    pub fn ruc(&self) -> String {
        self.sol.ruc.clone()
    }

    /// RUC del emisor de guías en la API GRE de prueba.
    pub fn gre_ruc(&self) -> String {
        self.gre
            .as_ref()
            .map_or_else(|| "20161515648".to_owned(), |gre| gre.ruc.clone())
    }

    /// `See` para facturas, boletas, notas, resúmenes y bajas (FE_BETA).
    ///
    /// # Errors
    ///
    /// Certificado no aceptado.
    pub fn see(&self) -> Result<See<ReqwestTransport>, Box<dyn Error>> {
        self.see_en(SunatEndpoints::FE_BETA)
    }

    /// `See` para percepción, retención y reversión (RETENCION_BETA).
    ///
    /// # Errors
    ///
    /// Certificado no aceptado.
    pub fn see_retencion(&self) -> Result<See<ReqwestTransport>, Box<dyn Error>> {
        self.see_en(SunatEndpoints::RETENCION_BETA)
    }

    fn see_en(&self, endpoint: &str) -> Result<See<ReqwestTransport>, Box<dyn Error>> {
        let mut see = See::new(self.transport.clone());
        see.set_certificate(&self.pem)?;
        see.set_service(endpoint);
        see.set_clave_sol(&self.sol.ruc, &self.sol.usuario, &self.sol.clave);
        Ok(see)
    }

    /// `Api` GRE de prueba; `None` si el JSON no tiene la sección `gre`.
    ///
    /// # Errors
    ///
    /// `gre.api_url` de producción.
    pub fn api(&self) -> Result<Option<ApiGre>, Box<dyn Error>> {
        let Some(gre) = &self.gre else {
            return Ok(None);
        };
        let url = gre.api_url.as_str();
        if url.contains("sunat.gob.pe") && !url.contains("test") && !url.contains("beta") {
            return Err(
                "gre.api_url apunta a producción; este ejemplo solo usa entornos de prueba".into(),
            );
        }
        let mut api = Api::new(ApiFactory::new(
            self.transport.clone(),
            Configuration::auth().with_host(url),
            url,
            InMemoryStore::default(),
            SystemClock,
        ));
        api.set_api_credentials(&gre.client_id, &gre.client_secret)
            .set_clave_sol(&gre.ruc, &gre.usuario, &gre.clave)
            .set_certificate(&self.pem);
        Ok(Some(api))
    }
}
