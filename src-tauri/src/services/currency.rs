//! Multi-currency helpers. Project financials are computed in the project's currency.
//! Salary / default currency may differ; amounts are converted via `exchange_rates`.

use crate::db::Db;
use crate::error::AppResult;
use crate::finance::convert_minor;
use mysql::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyInfo {
    pub code: String,
    pub name: String,
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRateDto {
    pub id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub rate_numeric: String,
    pub effective_on: String,
}

pub fn supported_currencies() -> Vec<CurrencyInfo> {
    vec![
        CurrencyInfo {
            code: "USD".into(),
            name: "US Dollar".into(),
            symbol: "$".into(),
        },
        CurrencyInfo {
            code: "PKR".into(),
            name: "Pakistani Rupee".into(),
            symbol: "Rs".into(),
        },
        CurrencyInfo {
            code: "EUR".into(),
            name: "Euro".into(),
            symbol: "€".into(),
        },
        CurrencyInfo {
            code: "GBP".into(),
            name: "British Pound".into(),
            symbol: "£".into(),
        },
        CurrencyInfo {
            code: "AED".into(),
            name: "UAE Dirham".into(),
            symbol: "AED".into(),
        },
        CurrencyInfo {
            code: "SAR".into(),
            name: "Saudi Riyal".into(),
            symbol: "SAR".into(),
        },
        CurrencyInfo {
            code: "INR".into(),
            name: "Indian Rupee".into(),
            symbol: "₹".into(),
        },
    ]
}

pub fn normalize(code: &str) -> String {
    code.trim().to_uppercase()
}

/// Quote units per 1 base unit (e.g. base=USD, quote=PKR, rate≈278).
pub fn get_rate(db: &Db, base: &str, quote: &str) -> AppResult<Option<String>> {
    let base = normalize(base);
    let quote = normalize(quote);
    if base == quote {
        return Ok(Some("1".into()));
    }
    db.with_conn(|conn| {
        let direct: Option<String> = conn.exec_first(
            "SELECT rate_numeric FROM exchange_rates
             WHERE base_currency = ? AND quote_currency = ?
             ORDER BY effective_on DESC LIMIT 1",
            (&base, &quote),
        )?;
        if direct.is_some() {
            return Ok(direct);
        }
        // Inverse: if we have quote→base, use 1/rate
        let inverse: Option<String> = conn.exec_first(
            "SELECT rate_numeric FROM exchange_rates
             WHERE base_currency = ? AND quote_currency = ?
             ORDER BY effective_on DESC LIMIT 1",
            (&quote, &base),
        )?;
        if let Some(r) = inverse {
            if let Ok(v) = r.parse::<f64>() {
                if v > 0.0 && v.is_finite() {
                    return Ok(Some(format!("{:.10}", 1.0 / v)));
                }
            }
        }
        // Pivot via USD when neither leg is USD
        if base != "USD" && quote != "USD" {
            let to_usd = get_rate_conn(conn, &base, "USD")?;
            let usd_to_quote = get_rate_conn(conn, "USD", &quote)?;
            if let (Some(a), Some(b)) = (to_usd, usd_to_quote) {
                let av: f64 = a.parse().unwrap_or(0.0);
                let bv: f64 = b.parse().unwrap_or(0.0);
                if av > 0.0 && bv > 0.0 {
                    return Ok(Some(format!("{:.10}", av * bv)));
                }
            }
        }
        Ok(None)
    })
}

fn get_rate_conn(
    conn: &mut mysql::PooledConn,
    base: &str,
    quote: &str,
) -> AppResult<Option<String>> {
    if base == quote {
        return Ok(Some("1".into()));
    }
    let direct: Option<String> = conn.exec_first(
        "SELECT rate_numeric FROM exchange_rates
         WHERE base_currency = ? AND quote_currency = ?
         ORDER BY effective_on DESC LIMIT 1",
        (base, quote),
    )?;
    if direct.is_some() {
        return Ok(direct);
    }
    let inverse: Option<String> = conn.exec_first(
        "SELECT rate_numeric FROM exchange_rates
         WHERE base_currency = ? AND quote_currency = ?
         ORDER BY effective_on DESC LIMIT 1",
        (quote, base),
    )?;
    if let Some(r) = inverse {
        if let Ok(v) = r.parse::<f64>() {
            if v > 0.0 && v.is_finite() {
                return Ok(Some(format!("{:.10}", 1.0 / v)));
            }
        }
    }
    Ok(None)
}

pub fn convert_amount(db: &Db, amount_minor: i64, from: &str, to: &str) -> AppResult<i64> {
    let from = normalize(from);
    let to = normalize(to);
    if from == to || amount_minor == 0 {
        return Ok(amount_minor);
    }
    let rate = get_rate(db, &from, &to)?.ok_or_else(|| {
        crate::error::AppError::Message(format!(
            "No exchange rate for {from} → {to}. Add one in Settings."
        ))
    })?;
    convert_minor(amount_minor, &rate).ok_or_else(|| {
        crate::error::AppError::Message(format!("Invalid exchange rate for {from} → {to}"))
    })
}

/// Convert when possible; if rate missing, return original and flag unavailable.
pub fn try_convert(db: &Db, amount_minor: i64, from: &str, to: &str) -> AppResult<(i64, bool)> {
    let from = normalize(from);
    let to = normalize(to);
    if from == to || amount_minor == 0 {
        return Ok((amount_minor, true));
    }
    match get_rate(db, &from, &to)? {
        Some(rate) => Ok((convert_minor(amount_minor, &rate).unwrap_or(amount_minor), true)),
        None => Ok((amount_minor, false)),
    }
}

pub fn list_rates(db: &Db) -> AppResult<Vec<ExchangeRateDto>> {
    db.with_conn(|conn| {
        let rows: Vec<(String, String, String, String, String)> = conn.query(
            "SELECT id, base_currency, quote_currency, rate_numeric, effective_on
             FROM exchange_rates ORDER BY effective_on DESC, base_currency, quote_currency",
        )?;
        Ok(rows
            .into_iter()
            .map(|(id, base, quote, rate, on)| ExchangeRateDto {
                id,
                base_currency: base,
                quote_currency: quote,
                rate_numeric: rate,
                effective_on: on,
            })
            .collect())
    })
}

pub fn upsert_rate(
    db: &Db,
    base: &str,
    quote: &str,
    rate_numeric: &str,
    effective_on: &str,
) -> AppResult<ExchangeRateDto> {
    let base = normalize(base);
    let quote = normalize(quote);
    let rate: f64 = rate_numeric.parse().map_err(|_| {
        crate::error::AppError::Message("Rate must be a positive number".into())
    })?;
    if !rate.is_finite() || rate <= 0.0 {
        return Err(crate::error::AppError::Message(
            "Rate must be a positive number".into(),
        ));
    }
    if base == quote {
        return Err(crate::error::AppError::Message(
            "Base and quote currency must differ".into(),
        ));
    }
    let id = Uuid::new_v4().to_string();
    let rate_s = rate_numeric.trim().to_string();
    db.with_conn(|conn| {
        // Replace same pair+date
        conn.exec_drop(
            "DELETE FROM exchange_rates WHERE base_currency = ? AND quote_currency = ? AND effective_on = ?",
            (&base, &quote, effective_on),
        )?;
        conn.exec_drop(
            "INSERT INTO exchange_rates (id, base_currency, quote_currency, rate_numeric, effective_on)
             VALUES (?, ?, ?, ?, ?)",
            (&id, &base, &quote, &rate_s, effective_on),
        )?;
        Ok(())
    })?;
    Ok(ExchangeRateDto {
        id,
        base_currency: base,
        quote_currency: quote,
        rate_numeric: rate_s,
        effective_on: effective_on.to_string(),
    })
}

pub fn seed_default_rates(conn: &mut mysql::PooledConn) -> AppResult<()> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    // Illustrative starter rates — confirm/update in Settings.
    let seeds = [
        ("USD", "PKR", "278.50"),
        ("USD", "EUR", "0.92"),
        ("USD", "GBP", "0.79"),
        ("USD", "AED", "3.6725"),
        ("USD", "SAR", "3.75"),
        ("USD", "INR", "83.50"),
    ];
    for (base, quote, rate) in seeds {
        let exists: Option<String> = conn.exec_first(
            "SELECT id FROM exchange_rates WHERE base_currency = ? AND quote_currency = ? LIMIT 1",
            (base, quote),
        )?;
        if exists.is_some() {
            continue;
        }
        let id = Uuid::new_v4().to_string();
        conn.exec_drop(
            "INSERT INTO exchange_rates (id, base_currency, quote_currency, rate_numeric, effective_on)
             VALUES (?, ?, ?, ?, ?)",
            (&id, base, quote, rate, &today),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test_db;

    #[test]
    fn converts_usd_to_pkr() {
        let db = open_test_db().unwrap();
        let pkr = convert_amount(&db, 100_00, "USD", "PKR").unwrap(); // $100
        // 100 USD * 278.50 = 27850 PKR → minor 2_785_000
        assert!(pkr > 2_000_000);
        let back = convert_amount(&db, pkr, "PKR", "USD").unwrap();
        assert!((back - 100_00).abs() < 50); // allow rounding
    }

    #[test]
    fn same_currency_noop() {
        let db = open_test_db().unwrap();
        assert_eq!(convert_amount(&db, 500, "PKR", "PKR").unwrap(), 500);
    }
}
