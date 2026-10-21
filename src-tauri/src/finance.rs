//! Decimal-safe financial arithmetic using integer minor units (e.g. cents).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LaborRate {
    pub monthly_salary_minor: i64,
    pub monthly_hours: f64,
    pub currency: String,
}

impl LaborRate {
    /// Hourly labor rate in minor units (truncated toward zero after rounding to nearest minor).
    pub fn hourly_minor(&self) -> Option<i64> {
        if self.monthly_hours <= 0.0 {
            return None;
        }
        Some(((self.monthly_salary_minor as f64) / self.monthly_hours).round() as i64)
    }

    pub fn per_minute_minor(&self) -> Option<i64> {
        self.hourly_minor().map(|h| (h as f64 / 60.0).round() as i64)
    }

    pub fn labor_cost_for_ms(&self, work_ms: i64) -> Option<i64> {
        let hourly = self.hourly_minor()?;
        let hours = (work_ms as f64) / 3_600_000.0;
        Some((hours * hourly as f64).round() as i64)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProjectFinancials {
    pub currency: String,
    pub contracted_budget_minor: i64,
    pub recognized_revenue_minor: i64,
    pub payments_received_minor: i64,
    pub labor_cost_minor: i64,
    pub ai_cost_minor: i64,
    pub other_expenses_minor: i64,
    pub subscription_allocation_minor: i64,
    /// Labor + AI allocation + other + subscription (all treated as project expenses).
    pub total_cost_minor: i64,
    /// Primary P/L: contracted payment − all expenses. None when no contracted payment is set.
    pub contract_profit_minor: Option<i64>,
    /// Contract margin % = contract_profit / contracted payment.
    pub contract_margin_pct: Option<f64>,
    /// AI share of contracted payment (0–100+).
    pub ai_of_contract_pct: Option<f64>,
    /// Labor share of contracted payment (0–100+).
    pub labor_of_contract_pct: Option<f64>,
    /// Recognized revenue − total expenses (secondary; useful when revenue is booked separately).
    pub economic_profit_minor: i64,
    /// Payments received − all expenses (labor included as expense).
    pub cash_flow_minor: i64,
    /// Alias of contract_margin_pct when a contract exists; else revenue margin.
    pub profit_margin_pct: Option<f64>,
    /// Contracted payment still available after all expenses. None when no contract is set.
    pub budget_remaining_minor: Option<i64>,
    pub cost_per_work_hour_minor: Option<i64>,
    pub hours_until_budget: Option<f64>,
}

pub fn project_cost(
    labor: i64,
    ai: i64,
    other: i64,
    subscription_allocation: i64,
) -> i64 {
    labor + ai + other + subscription_allocation
}

pub fn compute_project_financials(
    currency: &str,
    contracted_budget_minor: i64,
    recognized_revenue_minor: i64,
    payments_received_minor: i64,
    labor_cost_minor: i64,
    ai_cost_minor: i64,
    other_expenses_minor: i64,
    subscription_allocation_minor: i64,
    combined_work_ms: i64,
) -> ProjectFinancials {
    // Expenses always accumulate (labor + AI + other), even with no contracted payment.
    let total_cost_minor = project_cost(
        labor_cost_minor,
        ai_cost_minor,
        other_expenses_minor,
        subscription_allocation_minor,
    );
    // P/L only when a contracted payment exists — otherwise N/A (not a fake loss).
    let contract_profit_minor = if contracted_budget_minor > 0 {
        Some(contracted_budget_minor - total_cost_minor)
    } else {
        None
    };
    let economic_profit_minor = recognized_revenue_minor - total_cost_minor;
    let cash_flow_minor = payments_received_minor - total_cost_minor;

    let contract_margin_pct = contract_profit_minor.map(|p| {
        (p as f64 / contracted_budget_minor as f64) * 100.0
    });
    let ai_of_contract_pct = if contracted_budget_minor > 0 {
        Some((ai_cost_minor as f64 / contracted_budget_minor as f64) * 100.0)
    } else {
        None
    };
    let labor_of_contract_pct = if contracted_budget_minor > 0 {
        Some((labor_cost_minor as f64 / contracted_budget_minor as f64) * 100.0)
    } else {
        None
    };
    // Prefer contract margin only — no contracted payment means P/L is N/A.
    let profit_margin_pct = contract_margin_pct;

    let work_hours = combined_work_ms as f64 / 3_600_000.0;
    let cost_per_work_hour_minor = if work_hours > 0.0 {
        Some((total_cost_minor as f64 / work_hours).round() as i64)
    } else {
        None
    };
    let budget_remaining_minor = contract_profit_minor;
    let hours_until_budget = match (cost_per_work_hour_minor, budget_remaining_minor) {
        (Some(rate), Some(remaining)) if rate > 0 && remaining > 0 => {
            Some(remaining as f64 / rate as f64)
        }
        _ => None,
    };

    ProjectFinancials {
        currency: currency.to_string(),
        contracted_budget_minor,
        recognized_revenue_minor,
        payments_received_minor,
        labor_cost_minor,
        ai_cost_minor,
        other_expenses_minor,
        subscription_allocation_minor,
        total_cost_minor,
        contract_profit_minor,
        contract_margin_pct,
        ai_of_contract_pct,
        labor_of_contract_pct,
        economic_profit_minor,
        cash_flow_minor,
        profit_margin_pct,
        budget_remaining_minor,
        cost_per_work_hour_minor,
        hours_until_budget,
    }
}

/// Convert amount using a stored decimal rate string (quote per 1 base).
pub fn convert_minor(amount_minor: i64, rate: &str) -> Option<i64> {
    let r: f64 = rate.parse().ok()?;
    if !r.is_finite() || r <= 0.0 {
        return None;
    }
    Some((amount_minor as f64 * r).round() as i64)
}

pub fn format_money(minor: i64, currency: &str) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    let whole = abs / 100;
    let frac = abs % 100;
    format!("{sign}{currency} {whole}.{frac:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hourly_rate_from_salary() {
        let rate = LaborRate {
            monthly_salary_minor: 480_00,
            monthly_hours: 160.0,
            currency: "USD".into(),
        };
        // $480 / 160h = $3/h = 300 cents
        assert_eq!(rate.hourly_minor(), Some(300));
        assert_eq!(rate.labor_cost_for_ms(3_600_000), Some(300));
    }

    #[test]
    fn contract_profit_includes_labor_ai_and_other() {
        let f = compute_project_financials(
            "USD",
            120_000, // contracted payment
            70_000,
            50_000,
            10_000, // labor expense
            2_000,  // AI allocation
            1_000,  // other
            500,    // subscription alloc
            3_600_000,
        );
        assert_eq!(f.total_cost_minor, 13_500);
        // Primary P/L vs contracted payment
        assert_eq!(f.contract_profit_minor, Some(120_000 - 13_500));
        assert_eq!(f.budget_remaining_minor, Some(120_000 - 13_500));
        assert_eq!(f.economic_profit_minor, 70_000 - 13_500);
        // Labor is included in expenses for cash P/L too
        assert_eq!(f.cash_flow_minor, 50_000 - 13_500);
        assert!(f.contract_margin_pct.unwrap() > 80.0);
        assert!(f.ai_of_contract_pct.unwrap() > 0.0);
        assert!(f.labor_of_contract_pct.unwrap() > 0.0);
    }

    #[test]
    fn expenses_without_contract_are_tracked_but_pl_is_na() {
        let f = compute_project_financials("USD", 0, 0, 0, 100, 50_000, 0, 0, 0);
        assert_eq!(f.total_cost_minor, 50_100); // labor + AI still counted
        assert_eq!(f.contract_profit_minor, None);
        assert_eq!(f.budget_remaining_minor, None);
        assert_eq!(f.profit_margin_pct, None);
        assert_eq!(f.contract_margin_pct, None);
        assert_eq!(f.hours_until_budget, None);
    }

    #[test]
    fn convert_and_format() {
        assert_eq!(convert_minor(100, "1.1"), Some(110));
        assert_eq!(format_money(12345, "USD"), "USD 123.45");
        assert_eq!(format_money(-50, "USD"), "-USD 0.50");
    }
}
