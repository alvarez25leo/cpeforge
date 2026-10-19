//! Estado tributario de un envío o consulta, separado del éxito de transporte.
//!
//! `BaseResult::success` indica que el servicio respondió como se esperaba
//! (hubo ticket, hubo CDR), no que SUNAT haya aceptado el comprobante: un
//! `getStatus` con código `99` y CDR de rechazo, o un `sendSummary` sin
//! ticket, también terminan con `success == Some(true)`. Ese campo forma parte
//! del contrato de resultados y no cambia. [`TaxStatus`] clasifica esos mismos
//! resultados según su efecto tributario.
//!
//! Los rangos de códigos son los del catálogo de errores de SUNAT:
//!
//! | Código | Significado | [`TaxStatus`] |
//! | --- | --- | --- |
//! | `0` sin notas | Aceptado | [`TaxStatus::Accepted`] |
//! | `0` con notas, o `4000` en adelante | Aceptado con observaciones | [`TaxStatus::AcceptedWithObservations`] |
//! | `0100`–`1999` | Excepción: el envío no se procesó | [`TaxStatus::Exception`] |
//! | `2000`–`3999` | Rechazo | [`TaxStatus::Rejected`] |
//! | `98` (ticket) | En proceso | [`TaxStatus::Pending`] |
//! | Cualquier otro, ausente o no numérico | Sin conclusión | [`TaxStatus::Indeterminate`] |
//!
//! Un `Err` de un envío (timeout, conexión cortada, respuesta ilegible) es
//! siempre [`TaxStatus::Indeterminate`]: la solicitud pudo llegar a SUNAT.
//! Antes de repetir un envío hay que consultar su estado
//! ([`crate::services::ConsultCdrService`] o el ticket) para no duplicarlo.

use cpeforge_core::model::response::{
    BillResult, CdrResponse, ResponseError, StatusCdrResult, StatusResult, SummaryResult,
};

/// Efecto tributario de una respuesta de SUNAT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TaxStatus {
    /// SUNAT recibió el envío y aún no lo ha procesado (ticket o código `98`).
    Pending,
    /// Comprobante aceptado sin observaciones.
    Accepted,
    /// Comprobante aceptado con observaciones (notas o código `4000+`).
    AcceptedWithObservations,
    /// Comprobante rechazado (`2000`–`3999`); su número ya no puede reutilizarse.
    Rejected,
    /// Excepción (`0100`–`1999`): SUNAT no procesó este envío. Algunos códigos
    /// (por ejemplo `1032` o `1033`) indican que el comprobante ya estaba
    /// registrado; consultar su estado antes de reenviarlo.
    Exception,
    /// La respuesta no permite concluir nada: falta el CDR o el ticket, el
    /// código es desconocido o el transporte falló.
    Indeterminate,
}

impl TaxStatus {
    /// Clasifica un CDR por su `ResponseCode` y sus notas.
    #[must_use]
    pub fn from_cdr(cdr: &CdrResponse) -> Self {
        match numeric(cdr.code.as_deref()) {
            Some(0) if cdr.notes.as_ref().is_some_and(|notes| !notes.is_empty()) => {
                Self::AcceptedWithObservations
            }
            Some(0) => Self::Accepted,
            Some(code) => Self::from_error_code(code),
            None => Self::Indeterminate,
        }
    }

    /// Resultado de `BillSender::send`: decide el CDR; sin él, el código del
    /// error (por ejemplo, el de un fault).
    #[must_use]
    pub fn of_bill(result: &BillResult) -> Self {
        match &result.cdr_response {
            Some(cdr) => Self::from_cdr(cdr),
            None => from_error(result.base.error.as_ref()),
        }
    }

    /// Resultado de `SummarySender::send` o `GreSender::send`: con ticket,
    /// [`Self::Pending`]; con éxito pero sin ticket, [`Self::Indeterminate`].
    #[must_use]
    pub fn of_summary(result: &SummaryResult) -> Self {
        if result.base.success == Some(true) {
            match result.ticket.as_deref() {
                Some(ticket) if !ticket.trim().is_empty() => Self::Pending,
                _ => Self::Indeterminate,
            }
        } else {
            from_error(result.base.error.as_ref())
        }
    }

    /// Consulta de ticket (`ExtService::get_status`, `GreSender::status`).
    ///
    /// Con CDR decide el CDR, también cuando el código del ticket es `99`
    /// (procesado con errores). Sin CDR, `98` es [`Self::Pending`] y los demás
    /// casos se clasifican por el código del error.
    #[must_use]
    pub fn of_status(result: &StatusResult) -> Self {
        if let Some(cdr) = &result.bill.cdr_response {
            return Self::from_cdr(cdr);
        }
        if numeric(result.code.as_deref()) == Some(98) {
            return Self::Pending;
        }
        from_error(result.bill.base.error.as_ref())
    }

    /// Consulta por comprobante (`ConsultCdrService`). Solo el CDR permite
    /// concluir; los códigos propios del servicio de consulta no se
    /// interpretan.
    #[must_use]
    pub fn of_consult(result: &StatusCdrResult) -> Self {
        match &result.bill.cdr_response {
            Some(cdr) => Self::from_cdr(cdr),
            None => from_error(result.bill.base.error.as_ref()),
        }
    }

    /// Aceptado, con o sin observaciones.
    #[must_use]
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted | Self::AcceptedWithObservations)
    }

    /// SUNAT ya decidió sobre el comprobante (aceptado o rechazado).
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::AcceptedWithObservations | Self::Rejected
        )
    }

    fn from_error_code(code: u32) -> Self {
        match code {
            100..=1999 => Self::Exception,
            2000..=3999 => Self::Rejected,
            4000.. => Self::AcceptedWithObservations,
            _ => Self::Indeterminate,
        }
    }
}

/// Un error sin CDR solo concluye si su código es una excepción o un rechazo.
fn from_error(error: Option<&ResponseError>) -> TaxStatus {
    match error.and_then(|e| numeric(e.code.as_deref())) {
        Some(code @ 100..=3999) => TaxStatus::from_error_code(code),
        _ => TaxStatus::Indeterminate,
    }
}

/// Código formado solo por dígitos ASCII (se admiten espacios en los extremos).
fn numeric(code: Option<&str>) -> Option<u32> {
    let code = code?.trim();
    if code.is_empty() || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    code.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cdr(code: Option<&str>, notes: &[&str]) -> CdrResponse {
        CdrResponse {
            code: code.map(str::to_owned),
            notes: Some(notes.iter().map(|n| (*n).to_owned()).collect()),
            ..CdrResponse::default()
        }
    }

    #[test]
    fn cdr_codes() {
        assert_eq!(
            TaxStatus::from_cdr(&cdr(Some("0"), &[])),
            TaxStatus::Accepted
        );
        assert_eq!(
            TaxStatus::from_cdr(&cdr(Some("0"), &["4252 - Observación"])),
            TaxStatus::AcceptedWithObservations
        );
        assert_eq!(
            TaxStatus::from_cdr(&cdr(Some("4000"), &[])),
            TaxStatus::AcceptedWithObservations
        );
        assert_eq!(
            TaxStatus::from_cdr(&cdr(Some("2335"), &[])),
            TaxStatus::Rejected
        );
        assert_eq!(
            TaxStatus::from_cdr(&cdr(Some("0100"), &[])),
            TaxStatus::Exception
        );
        assert_eq!(
            TaxStatus::from_cdr(&cdr(Some("1999"), &[])),
            TaxStatus::Exception
        );
        for unknown in [
            None,
            Some(""),
            Some("99"),
            Some("abc"),
            Some("-1"),
            Some("0x10"),
        ] {
            assert_eq!(
                TaxStatus::from_cdr(&cdr(unknown, &[])),
                TaxStatus::Indeterminate,
                "{unknown:?}"
            );
        }
    }

    #[test]
    fn helpers() {
        assert!(TaxStatus::AcceptedWithObservations.is_accepted());
        assert!(!TaxStatus::Pending.is_final());
        assert!(TaxStatus::Rejected.is_final());
        assert!(!TaxStatus::Exception.is_final());
    }
}
