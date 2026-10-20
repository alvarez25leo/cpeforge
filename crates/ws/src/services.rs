//! Servicios `BillSender`, `SummarySender`, `ExtService` y `ConsultCdrService`.
//!
//! Los resultados distinguen tres niveles:
//!
//! - `Err(WsError)`: no hay respuesta interpretable (transporte, envelope o
//!   CDR ilegibles). El envío pudo llegar a SUNAT; ningún servicio reintenta.
//! - `Ok` con `base.error`: SUNAT respondió con un fault o un código de error,
//!   o faltó el CDR esperado.
//! - `Ok` con `base.success == Some(true)`: el servicio respondió lo esperado.
//!   No equivale a aceptación tributaria; para eso, [`crate::outcome::TaxStatus`].

use cpeforge_core::model::response::{
    BillResult, ResponseError, StatusCdrResult, StatusResult, SummaryResult,
};

use crate::WsError;
use crate::cdr::read_cdr_zip;
use crate::soap::{SoapStatus, WsClient};
use crate::zip::{ZipTime, compress_at};

const NO_CDR_BILL: &str =
    "CDR no encontrado. La respuesta del servicio de SUNAT no incluyó el CDR.";
const NO_CDR_STATUS: &str = "CDR no encontrado en código estado: 98 (Procesado).";

/// Código numérico de un fault: los dígitos del `faultcode` (`soap:Client.0156`
/// da `0156`) o, si no hay, los del `faultstring`. Un resultado `"0"` cuenta
/// como ausente, igual que una cadena vacía. `None` si tampoco quedan dígitos.
#[must_use]
pub fn fault_code_digits(code: &str, message: &str) -> Option<String> {
    let digits = |text: &str| -> String { text.chars().filter(char::is_ascii_digit).collect() };
    let meaningful = |text: &str| !text.is_empty() && text != "0";
    let mut found = digits(code);
    if !meaningful(&found) && meaningful(message) {
        found = digits(message);
    }
    meaningful(&found).then_some(found)
}

/// Convierte un fault en el error del resultado: código según
/// [`fault_code_digits`] (o el `faultcode` literal) y mensaje igual al
/// `faultstring`, seguido de `" | "` y el JSON del `detail` si lo hay.
/// Cualquier otro error se propaga.
fn fault_error(error: WsError) -> Result<ResponseError, WsError> {
    match error {
        WsError::SoapFault {
            code,
            message,
            detail,
        } => {
            let code = fault_code_digits(&code, &message).unwrap_or(code);
            let message = match detail {
                Some(detail) => format!("{message} | {detail}"),
                None => message,
            };
            Ok(ResponseError::new(Some(code), Some(message)))
        }
        other => Err(other),
    }
}

/// Código de excepción de SUNAT (`100`–`1999`).
fn is_exception(code: Option<&str>) -> bool {
    code.and_then(|s| s.parse::<i32>().ok())
        .is_some_and(|n| (100..=1999).contains(&n))
}

/// Envía un CPE individual por SOAP `sendBill`.
pub struct BillSender<C> {
    client: C,
    zip_time: ZipTime,
}

impl<C> std::fmt::Debug for BillSender<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BillSender")
            .field("zip_time", &self.zip_time)
            .finish_non_exhaustive()
    }
}

impl<C: WsClient> BillSender<C> {
    /// Inyecta un cliente SOAP real o simulado.
    #[must_use]
    pub fn new(client: C) -> Self {
        Self {
            client,
            zip_time: ZipTime::default(),
        }
    }

    /// Hora que se escribe en el ZIP (por defecto, la actual en UTC).
    #[must_use]
    pub fn with_zip_time(mut self, zip_time: ZipTime) -> Self {
        self.zip_time = zip_time;
        self
    }

    /// Comprime `<filename>.xml`, envía `<filename>.zip` y lee el CDR.
    ///
    /// # Errors
    ///
    /// ZIP/CDR inválido, transporte fallido o respuesta incompleta.
    pub async fn send(&self, filename: &str, xml: &[u8]) -> Result<BillResult, WsError> {
        let zipped = compress_at(&format!("{filename}.xml"), xml, self.zip_time)?;
        let mut result = BillResult::default();
        match self
            .client
            .send_bill(&format!("{filename}.zip"), &zipped)
            .await
        {
            Ok(Some(cdr_zip)) if !cdr_zip.is_empty() => {
                result.cdr_response = Some(read_cdr_zip(&cdr_zip)?);
                result.cdr_zip = Some(cdr_zip);
                result.base.success = Some(true);
            }
            Ok(_) => {
                result.base.error = Some(ResponseError::new(
                    Some("CDR".into()),
                    Some(NO_CDR_BILL.into()),
                ))
            }
            Err(error) => result.base.error = Some(fault_error(error)?),
        }
        Ok(result)
    }
}

/// Envía un resumen/baja por SOAP `sendSummary`.
pub struct SummarySender<C> {
    client: C,
    zip_time: ZipTime,
}

impl<C> std::fmt::Debug for SummarySender<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SummarySender")
            .field("zip_time", &self.zip_time)
            .finish_non_exhaustive()
    }
}

impl<C: WsClient> SummarySender<C> {
    /// Inyecta cliente.
    #[must_use]
    pub fn new(client: C) -> Self {
        Self {
            client,
            zip_time: ZipTime::default(),
        }
    }

    /// Hora que se escribe en el ZIP (por defecto, la actual en UTC).
    #[must_use]
    pub fn with_zip_time(mut self, zip_time: ZipTime) -> Self {
        self.zip_time = zip_time;
        self
    }

    /// Envía ZIP y devuelve el ticket.
    ///
    /// Si el servicio responde sin elemento de respuesta o sin ticket, el
    /// resultado conserva `success == Some(true)` con `ticket == None`: hubo
    /// respuesta, pero no puede consultarse el estado.
    /// [`crate::outcome::TaxStatus::of_summary`] lo clasifica como
    /// `Indeterminate`.
    ///
    /// # Errors
    ///
    /// ZIP o transporte inválido.
    pub async fn send(&self, filename: &str, xml: &[u8]) -> Result<SummaryResult, WsError> {
        let zipped = compress_at(&format!("{filename}.xml"), xml, self.zip_time)?;
        let mut result = SummaryResult::default();
        match self
            .client
            .send_summary(&format!("{filename}.zip"), &zipped)
            .await
        {
            Ok(ticket) => {
                result.ticket = ticket;
                result.base.success = Some(true);
            }
            Err(error) => result.base.error = Some(fault_error(error)?),
        }
        Ok(result)
    }
}

/// Consulta un ticket SOAP del servicio de facturación.
pub struct ExtService<C> {
    client: C,
}

impl<C> std::fmt::Debug for ExtService<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtService").finish_non_exhaustive()
    }
}

impl<C: WsClient> ExtService<C> {
    /// Inyecta cliente.
    #[must_use]
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// `getStatus`: `98` pendiente, `0`/`99` procesado (con CDR), `100`–`1999`
    /// excepción. Un código desconocido deja el resultado sin éxito ni error.
    ///
    /// # Errors
    ///
    /// Respuesta incompleta, CDR inválido o transporte fallido.
    pub async fn get_status(&self, ticket: &str) -> Result<StatusResult, WsError> {
        let status = match self.client.get_status(ticket).await {
            Ok(Some(status)) => status,
            Ok(None) => {
                return Err(WsError::InvalidServiceResponse(
                    "Invalid getStatus service response.",
                ));
            }
            Err(error) => {
                let mut result = StatusResult::default();
                result.bill.base.error = Some(fault_error(error)?);
                return Ok(result);
            }
        };
        ext_result(status)
    }
}

fn ext_result(status: SoapStatus) -> Result<StatusResult, WsError> {
    let mut result = StatusResult {
        code: status.code.clone(),
        ..StatusResult::default()
    };
    let code = status.code.as_deref().unwrap_or_default();
    match code {
        "98" => {
            result.bill.base.error = Some(ResponseError::new(
                Some(code.into()),
                Some("El procesamiento del comprobante aún no ha terminado".into()),
            ))
        }
        "0" | "99" => match status.content {
            Some(zip) if !zip.is_empty() => {
                let cdr = read_cdr_zip(&zip)?;
                if is_exception(cdr.code.as_deref()) {
                    result.bill.base.error =
                        Some(ResponseError::new(status.code, cdr.description.clone()));
                } else {
                    result.bill.base.success = Some(true);
                }
                result.bill.cdr_response = Some(cdr);
                result.bill.cdr_zip = Some(zip);
            }
            _ => {
                result.bill.base.error = Some(ResponseError::new(
                    Some("CDR".into()),
                    Some(NO_CDR_STATUS.into()),
                ))
            }
        },
        other if is_exception(Some(other)) => {
            result.bill.base.error = Some(ResponseError::with_code(Some(other.into())))
        }
        _ => {}
    }
    Ok(result)
}

/// Consulta estado y CDR por RUC, tipo, serie y número.
pub struct ConsultCdrService<C> {
    client: C,
}

impl<C> std::fmt::Debug for ConsultCdrService<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsultCdrService").finish_non_exhaustive()
    }
}

impl<C: WsClient> ConsultCdrService<C> {
    /// Inyecta cliente.
    #[must_use]
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// SOAP `getStatus`.
    ///
    /// # Errors
    ///
    /// Respuesta incompleta o transporte fallido.
    pub async fn get_status(
        &self,
        ruc: &str,
        tipo: &str,
        serie: &str,
        numero: i32,
    ) -> Result<StatusCdrResult, WsError> {
        self.query(false, ruc, tipo, serie, numero).await
    }

    /// SOAP `getStatusCdr`.
    ///
    /// # Errors
    ///
    /// Respuesta/CDR incompleto o transporte fallido.
    pub async fn get_status_cdr(
        &self,
        ruc: &str,
        tipo: &str,
        serie: &str,
        numero: i32,
    ) -> Result<StatusCdrResult, WsError> {
        self.query(true, ruc, tipo, serie, numero).await
    }

    async fn query(
        &self,
        cdr: bool,
        ruc: &str,
        tipo: &str,
        serie: &str,
        numero: i32,
    ) -> Result<StatusCdrResult, WsError> {
        let mut result = StatusCdrResult::default();
        match self.client.consult(cdr, ruc, tipo, serie, numero).await {
            Ok(Some(status)) => {
                result.code = status.code;
                result.message = status.message;
                result.bill.base.success = Some(true);
                if let Some(zip) = status.content {
                    result.bill.cdr_response = Some(read_cdr_zip(&zip)?);
                    result.bill.cdr_zip = Some(zip);
                }
                let code = result
                    .bill
                    .cdr_response
                    .as_ref()
                    .and_then(|v| v.code.as_deref())
                    .or(result.code.as_deref());
                if is_exception(code) {
                    result.bill.base.success = Some(false);
                    result.bill.base.error = Some(ResponseError::new(
                        code.map(str::to_owned),
                        result
                            .bill
                            .cdr_response
                            .as_ref()
                            .and_then(|v| v.description.clone()),
                    ));
                }
            }
            Ok(None) => return Err(WsError::InvalidResponse("status de consulta ausente")),
            Err(error) => result.bill.base.error = Some(fault_error(error)?),
        }
        Ok(result)
    }
}
