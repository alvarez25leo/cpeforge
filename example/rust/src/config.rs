//! Configuración de `See` y `Api`, sin credenciales en el código:
//! se leen de `cert/` en la raíz del repositorio (o de las rutas en
//! `EJEMPLO_CERT` y `EJEMPLO_CREDENCIALES`).
//!
//! Los servicios quedan fijos en los entornos de prueba (SUNAT BETA y la API
//! GRE de `GRE_API_URL`): este ejemplo nunca usa producción.

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use cpeforge::{Api, See};
use cpeforge_gre_api::{Configuration, ReqwestTransport};
use cpeforge_ws::api::{ApiFactory, InMemoryStore, SystemClock};
use cpeforge_ws::soap::SunatEndpoints;

/// `Api` GRE con transporte real, almacén de token en memoria y reloj del sistema.
pub type ApiGre = Api<ReqwestTransport, InMemoryStore, SystemClock>;

/// Certificado y credenciales leídos de `cert/`.
pub struct Config {
    pem: String,
    cred: HashMap<String, String>,
    transport: ReqwestTransport,
}

/// Lee un texto con líneas `Clave: valor` o `CLAVE= valor`; gana la primera.
fn leer_credenciales(path: &Path) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let texto = fs::read_to_string(path).map_err(|e| {
        format!(
            "No se pudo leer el archivo de credenciales {}: {e}",
            path.display()
        )
    })?;
    let mut out = HashMap::new();
    for linea in texto.lines() {
        let Some(corte) = linea.find([':', '=']) else {
            continue;
        };
        let (clave, valor) = (linea[..corte].trim(), linea[corte + 1..].trim());
        if !clave.is_empty()
            && !valor.is_empty()
            && clave.chars().all(|c| c.is_alphabetic() || c == '_')
        {
            out.entry(clave.to_owned())
                .or_insert_with(|| valor.to_owned());
        }
    }
    for clave in ["RUC", "Usuario", "Contraseña"] {
        if !out.contains_key(clave) {
            return Err(format!("Falta «{clave}:» en {}", path.display()).into());
        }
    }
    Ok(out)
}

fn ruta(variable: &str, por_defecto: &str) -> PathBuf {
    std::env::var_os(variable).map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(por_defecto)
        },
        PathBuf::from,
    )
}

impl Config {
    /// Lee el certificado y las credenciales.
    ///
    /// # Errors
    ///
    /// Archivos ilegibles, credenciales incompletas o fallo del transporte TLS.
    pub fn cargar() -> Result<Self, Box<dyn Error>> {
        let cert_path = ruta("EJEMPLO_CERT", "cert/certificate.pem");
        let pem = fs::read_to_string(&cert_path).map_err(|e| {
            format!(
                "No se pudo leer el certificado {}: {e}",
                cert_path.display()
            )
        })?;
        let cred = leer_credenciales(&ruta("EJEMPLO_CREDENCIALES", "cert/user.txt"))?;
        Ok(Self {
            pem,
            cred,
            transport: ReqwestTransport::new()?,
        })
    }

    fn get(&self, clave: &str) -> &str {
        self.cred.get(clave).map_or("", String::as_str)
    }

    /// RUC de la Clave SOL.
    pub fn ruc(&self) -> String {
        self.get("RUC").to_owned()
    }

    /// RUC del emisor de guías en la API GRE de prueba.
    pub fn gre_ruc(&self) -> String {
        self.cred
            .get("GRE_RUC")
            .cloned()
            .unwrap_or_else(|| "20161515648".to_owned())
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
        see.set_clave_sol(self.get("RUC"), self.get("Usuario"), self.get("Contraseña"));
        Ok(see)
    }

    /// `Api` GRE de prueba; `None` si faltan sus credenciales.
    ///
    /// # Errors
    ///
    /// `GRE_API_URL` de producción.
    pub fn api(&self) -> Result<Option<ApiGre>, Box<dyn Error>> {
        let claves = [
            "GRE_API_URL",
            "GRE_CLIENT_ID",
            "GRE_CLIENT_SECRET",
            "GRE_RUC",
            "GRE_USUARIO",
            "GRE_CLAVE",
        ];
        if claves.iter().any(|clave| !self.cred.contains_key(*clave)) {
            return Ok(None);
        }
        let url = self.get("GRE_API_URL");
        if url.contains("sunat.gob.pe") && !url.contains("test") && !url.contains("beta") {
            return Err(
                "GRE_API_URL apunta a producción; este ejemplo solo usa entornos de prueba".into(),
            );
        }
        let mut api = Api::new(ApiFactory::new(
            self.transport.clone(),
            Configuration::auth().with_host(url),
            url,
            InMemoryStore::default(),
            SystemClock,
        ));
        api.set_api_credentials(self.get("GRE_CLIENT_ID"), self.get("GRE_CLIENT_SECRET"))
            .set_clave_sol(
                self.get("GRE_RUC"),
                self.get("GRE_USUARIO"),
                self.get("GRE_CLAVE"),
            )
            .set_certificate(&self.pem);
        Ok(Some(api))
    }
}
