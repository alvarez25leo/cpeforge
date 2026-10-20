//! `FeFactory`: orden de construcción, firma y envío.

use std::future::Future;
use std::pin::Pin;

use cpeforge_core::model::response::{BillResult, SummaryResult};
use cpeforge_xmldsig::sunat::SignedXml;

use crate::CpeForgeError;
use crate::resolver::{DocumentRef, XmlBuilder};

/// Resultado de los dos tipos de sender.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SendResult {
    /// `BillSender`, con CDR.
    Bill(BillResult),
    /// `SummarySender` o `GreSender`, con ticket.
    Summary(SummaryResult),
}

/// Firma inyectable. Implementado por el `SignedXml` del crate existente.
pub trait XmlSigner: Send + Sync {
    /// Firma el XML construido.
    fn sign_xml(&self, xml: &str) -> Result<String, CpeForgeError>;
}

impl XmlSigner for SignedXml {
    fn sign_xml(&self, xml: &str) -> Result<String, CpeForgeError> {
        SignedXml::sign_xml(self, xml).map_err(Into::into)
    }
}

/// Sender inyectable para SOAP o dobles sin red.
pub trait Sender: Send + Sync {
    /// Envía exactamente los bytes recibidos; comprimirlos corresponde a `ws`.
    fn send<'a>(
        &'a self,
        filename: &'a str,
        xml: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<SendResult, CpeForgeError>> + Send + 'a>>;
}

/// Factory mutable: builder, firmador y sender sustituibles.
#[derive(Default)]
pub struct FeFactory {
    builder: Option<Box<dyn XmlBuilder>>,
    signer: Option<Box<dyn XmlSigner>>,
    sender: Option<Box<dyn Sender>>,
    last_xml: Option<String>,
}

impl std::fmt::Debug for FeFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeFactory")
            .field("builder", &self.builder.is_some())
            .field("signer", &self.signer.is_some())
            .field("sender", &self.sender.is_some())
            .field("last_xml", &self.last_xml.as_ref().map(String::len))
            .finish()
    }
}

impl FeFactory {
    /// Factory vacío, igual que `new FeFactory()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reemplaza o elimina el builder.
    pub fn set_builder(&mut self, builder: Option<Box<dyn XmlBuilder>>) -> &mut Self {
        self.builder = builder;
        self
    }
    /// Reemplaza o elimina el firmador.
    pub fn set_signer(&mut self, signer: Option<Box<dyn XmlSigner>>) -> &mut Self {
        self.signer = signer;
        self
    }
    /// Reemplaza o elimina el sender.
    pub fn set_sender(&mut self, sender: Option<Box<dyn Sender>>) -> &mut Self {
        self.sender = sender;
        self
    }
    /// Builder actual.
    #[must_use]
    pub fn builder(&self) -> Option<&dyn XmlBuilder> {
        self.builder.as_deref()
    }
    /// Firmador actual.
    #[must_use]
    pub fn signer(&self) -> Option<&dyn XmlSigner> {
        self.signer.as_deref()
    }
    /// Sender actual.
    #[must_use]
    pub fn sender(&self) -> Option<&dyn Sender> {
        self.sender.as_deref()
    }
    /// Último XML firmado, aun si después falló el envío.
    #[must_use]
    pub fn last_xml(&self) -> Option<&str> {
        self.last_xml.as_deref()
    }

    /// Construye y firma; actualiza `lastXml` solo cuando la firma termina.
    pub fn get_xml_signed(&mut self, document: DocumentRef<'_>) -> Result<&str, CpeForgeError> {
        let xml = self
            .builder
            .as_ref()
            .ok_or(CpeForgeError::Missing("builder"))?
            .build(document)?;
        let signed = self
            .signer
            .as_ref()
            .ok_or(CpeForgeError::Missing("signer"))?
            .sign_xml(&xml)?;
        self.last_xml = Some(signed);
        self.last_xml
            .as_deref()
            .ok_or(CpeForgeError::Missing("lastXml"))
    }

    /// Genera, firma, obtiene el nombre y envía, en ese orden.
    pub async fn send(&mut self, document: DocumentRef<'_>) -> Result<SendResult, CpeForgeError> {
        self.get_xml_signed(document)?;
        let name = document.name()?;
        let sender = self
            .sender
            .as_ref()
            .ok_or(CpeForgeError::Missing("sender"))?;
        let xml = self
            .last_xml
            .as_deref()
            .ok_or(CpeForgeError::Missing("lastXml"))?;
        sender.send(&name, xml.as_bytes()).await
    }

    /// Envía XML del llamador sin builder ni firma y sin cambiar `lastXml`.
    pub async fn send_xml(&self, name: &str, xml: &[u8]) -> Result<SendResult, CpeForgeError> {
        self.sender
            .as_ref()
            .ok_or(CpeForgeError::Missing("sender"))?
            .send(name, xml)
            .await
    }
}
