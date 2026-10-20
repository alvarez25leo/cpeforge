//! Fachada `Api` de GRE. Red, token, ZIP y CDR provienen de `ws`/`gre-api`.

use cpeforge_core::model::response::{StatusResult, SummaryResult};
use cpeforge_gre_api::{Configuration, HttpTransport, Secret, TokenRequest};
use cpeforge_ws::api::{ApiFactory, Clock, GreSender, TokenStore};
use cpeforge_ws::zip::ZipTime;
use cpeforge_xml::RenderOptions;
use cpeforge_xmldsig::sunat::SignedXml;

use crate::CpeForgeError;
use crate::options::BuilderOptionsPatch;
use crate::resolver::{DocumentRef, XmlBuilder, XmlBuilderResolver};

/// Host `auth` predeterminado de la API GRE.
pub const DEFAULT_AUTH_HOST: &str = "https://api-seguridad.sunat.gob.pe/v1";
/// Host `cpe` predeterminado de la API GRE.
pub const DEFAULT_CPE_HOST: &str = "https://api-cpe.sunat.gob.pe/v1";

/// Fachada para la API GRE. El store y reloj residen en `ws::ApiFactory`.
///
/// Los tokens se guardan en el [`TokenStore`] de la factory con la clave
/// `CacheKey::Scoped` por defecto (host, `scope`, `client_id` y usuario SOL),
/// así que una tienda compartida entre varias empresas no mezcla sus tokens.
/// Para otra clave, crear la factory con `ApiFactory::with_cache_key` y
/// pasarla a [`Api::new`].
pub struct Api<T, S, C> {
    factory: ApiFactory<T, S, C>,
    signer: SignedXml,
    options: RenderOptions,
    client_id: Option<String>,
    client_secret: Option<Secret>,
    username: Option<String>,
    password: Option<Secret>,
    last_xml: Option<String>,
    zip_time: ZipTime,
}

impl<T, S, C> std::fmt::Debug for Api<T, S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Api")
            .field("factory", &"<oculta>")
            .field("signer", &"<oculto>")
            .field("credentials", &"<ocultas>")
            .field("last_xml", &self.last_xml.as_ref().map(String::len))
            .finish()
    }
}

impl<T: HttpTransport + Clone, S: TokenStore, C: Clock> Api<T, S, C> {
    /// Inyecta la factory de `ws`. Ninguna llamada de red ocurre aquí.
    #[must_use]
    pub fn new(factory: ApiFactory<T, S, C>) -> Self {
        Self {
            factory,
            signer: SignedXml::new(),
            options: RenderOptions::cpeforge(),
            client_id: None,
            client_secret: None,
            username: None,
            password: None,
            last_xml: None,
            zip_time: ZipTime::default(),
        }
    }

    /// Crea la factory de `ws` con los hosts predeterminados.
    #[must_use]
    pub fn with_defaults(transport: T, store: S, clock: C) -> Self {
        Self::new(ApiFactory::new(
            transport,
            Configuration::auth().with_host(DEFAULT_AUTH_HOST),
            DEFAULT_CPE_HOST,
            store,
            clock,
        ))
    }

    /// Sustituye el firmador, útil para una clave efímera de prueba.
    pub fn set_signer(&mut self, signer: SignedXml) -> &mut Self {
        self.signer = signer;
        self
    }
    /// Mezcla las opciones indicadas con las actuales.
    pub fn set_builder_options(&mut self, options: BuilderOptionsPatch) -> &mut Self {
        options.apply(&mut self.options);
        self
    }
    /// Identidad de la aplicación GRE.
    pub fn set_api_credentials(&mut self, client_id: &str, secret: &str) -> &mut Self {
        self.client_id = Some(client_id.to_owned());
        self.client_secret = Some(Secret::new(secret));
        self
    }
    /// RUC+usuario secundario y clave SOL.
    pub fn set_clave_sol(&mut self, ruc: &str, user: &str, password: &str) -> &mut Self {
        self.username = Some(format!("{ruc}{user}"));
        self.password = Some(Secret::new(password));
        self
    }
    /// Configura PEM con certificado y clave; se valida al firmar.
    pub fn set_certificate(&mut self, pem: &str) -> &mut Self {
        self.signer.set_certificate(pem);
        self
    }
    /// Hora del ZIP enviado; por defecto, la hora actual en UTC.
    pub fn set_zip_time(&mut self, zip_time: ZipTime) -> &mut Self {
        self.zip_time = zip_time;
        self
    }
    /// Último XML firmado por `send`. `send_xml` y `get_status` nunca lo cambian.
    #[must_use]
    pub fn last_xml(&self) -> Option<&str> {
        self.last_xml.as_deref()
    }

    fn request(&self) -> Result<TokenRequest, CpeForgeError> {
        Ok(TokenRequest::password_grant(
            self.client_id
                .as_deref()
                .ok_or(CpeForgeError::Missing("client_id"))?,
            self.client_secret
                .clone()
                .ok_or(CpeForgeError::Missing("client_secret"))?,
            self.username
                .as_deref()
                .ok_or(CpeForgeError::Missing("username"))?,
            self.password
                .clone()
                .ok_or(CpeForgeError::Missing("password"))?,
        ))
    }

    async fn sender(&self) -> Result<GreSender<T>, CpeForgeError> {
        Ok(GreSender::new(self.factory.create(&self.request()?).await?)
            .with_zip_time(self.zip_time))
    }

    /// Construye, firma, actualiza `lastXml` y envía. Si falla el envío,
    /// conserva el XML firmado.
    pub async fn send(
        &mut self,
        document: DocumentRef<'_>,
    ) -> Result<SummaryResult, CpeForgeError> {
        let xml = XmlBuilderResolver::new(self.options.clone())
            .find(document.kind())
            .build(document)?;
        let signed = self.signer.sign_xml(&xml)?;
        self.last_xml = Some(signed);
        let name = document.name()?;
        let xml = self
            .last_xml
            .as_deref()
            .ok_or(CpeForgeError::Missing("lastXml"))?;
        self.send_xml(&name, xml.as_bytes()).await
    }

    /// Envía bytes XML del llamador sin construir ni firmar, y sin cambiar
    /// `lastXml`. `ws::GreSender` hace el ZIP y el HTTP.
    pub async fn send_xml(&self, name: &str, xml: &[u8]) -> Result<SummaryResult, CpeForgeError> {
        Ok(self.sender().await?.send(name, xml).await?)
    }

    /// Consulta un ticket GRE con el mismo token store de `ws`.
    pub async fn get_status(&self, ticket: &str) -> Result<StatusResult, CpeForgeError> {
        Ok(self.sender().await?.status(ticket).await?)
    }
}
