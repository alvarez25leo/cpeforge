//! Tipo esperado de cada propiedad con `Valid` en los loaders fijados.

use serde_json::Value;

pub(crate) fn nested_type(parent: &str, field: &str, value: &Value) -> Option<&'static str> {
    match (parent, field) {
        ("Client" | "Company", "address") => Some("Address"),
        ("Despatch", "destinatario" | "tercero") => Some("Client"),
        ("Despatch", "company") => Some("Company"),
        ("Despatch", "envio") => Some("Shipment"),
        ("Despatch", "docBaja" | "relDoc") => Some("Document"),
        ("Despatch", "details") => Some("DespatchDetail"),
        ("EmbededDespatch", "llegada" | "partida") => Some("Direction"),
        ("EmbededDespatch", "transportista") => Some("Client"),
        ("Invoice" | "Note", "client" | "seller") => Some("Client"),
        (
            "Invoice" | "Note" | "Perception" | "Retention" | "Reversion" | "Summary" | "Voided",
            "company",
        ) => Some("Company"),
        ("Invoice" | "Note", "details") => Some("SaleDetail"),
        ("Invoice" | "Note", "legends") => Some("Legend"),
        ("Invoice" | "Note", "guias" | "relDocs") => Some("Document"),
        ("Invoice", "anticipos") => Some("Prepayment"),
        ("Invoice", "detraccion") => Some("Detraction"),
        ("Invoice" | "Note", "perception") => Some("SalePerception"),
        ("Invoice", "guiaEmbebida") => Some("EmbededDespatch"),
        ("Invoice" | "Note", "formaPago") => match value.get("tipo").and_then(Value::as_str) {
            Some("Contado") => Some("FormaPagoContado"),
            Some("Credito") => Some("FormaPagoCredito"),
            _ => Some("PaymentTerms"),
        },
        ("Invoice" | "Note", "cuotas") => Some("Cuota"),
        ("Invoice" | "SaleDetail", "cargos" | "descuentos") => Some("Charge"),
        ("SaleDetail", "atributos") => Some("DetailAttribute"),
        ("Perception" | "Retention", "proveedor") => Some("Client"),
        ("Perception", "details") => Some("PerceptionDetail"),
        ("Retention", "details") => Some("RetentionDetail"),
        ("PerceptionDetail", "cobros") | ("RetentionDetail", "pagos") => Some("Payment"),
        ("PerceptionDetail" | "RetentionDetail", "tipoCambio") => Some("Exchange"),
        ("Reversion" | "Voided", "details") => Some("VoidedDetail"),
        ("Shipment", "transportista") => Some("Transportist"),
        ("Shipment", "llegada" | "partida") => Some("Direction"),
        ("Summary", "details") => Some("SummaryDetail"),
        ("SummaryDetail", "docReferencia") => Some("Document"),
        ("SummaryDetail", "percepcion") => Some("SummaryPerception"),
        _ => None,
    }
}
