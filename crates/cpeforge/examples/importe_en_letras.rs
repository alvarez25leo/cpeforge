//! Importe en letras para la leyenda 1000 (el mismo código del README).
//!
//! `cargo run -p cpeforge --example importe_en_letras`

use cpeforge::amount_words::{self, AmountWordsError};
use cpeforge_core::Decimal;
use cpeforge_core::model::sale::{BaseSale, Legend};

fn main() -> Result<(), AmountWordsError> {
    let total = Decimal::new(123_456_789, 2); // 1 234 567.89

    // Leyenda lista para el comprobante, a partir del código de moneda.
    let legend = Legend::amount_in_words(total, "PEN")?;
    assert_eq!(legend.code.as_deref(), Some("1000"));
    assert_eq!(
        legend.value.as_deref(),
        Some(
            "UN MILLÓN DOSCIENTOS TREINTA Y CUATRO MIL QUINIENTOS SESENTA Y SIETE CON 89/100 SOLES"
        )
    );

    let venta = BaseSale {
        tipo_moneda: Some("PEN".into()),
        mto_imp_venta: Some(total),
        legends: Some(vec![legend]),
        ..BaseSale::default()
    };
    assert_eq!(venta.legends.as_ref().map(Vec::len), Some(1));

    // Solo el texto, con el nombre de moneda que se quiera imprimir.
    let texto = amount_words::to_invoice(Decimal::new(21_000, 0), "dólares americanos")?;
    assert_eq!(texto, "VEINTIÚN MIL CON 00/100 DÓLARES AMERICANOS");
    println!("{texto}");
    Ok(())
}
