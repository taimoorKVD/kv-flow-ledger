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
    pub total_cost_minor: i64,
    /// Economic profit = recognized revenue − total cost (includes labor valuation).
    pub economic_profit_minor: i64,
    /// Cash flow = payments received − cash expenses (excludes imputed labor).
    pub cash_flow_minor: i64,
    pub profit_margin_pct: Option<f64>,
    pub budget_remaining_minor: i64,
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
    let total_cost_minor = project_cost(
        labor_cost_minor,
        ai_cost_minor,
        other_expenses_minor,
        subscription_allocation_minor,
    );
    let economic_profit_minor = recognized_revenue_minor - total_cost_minor;
    let cash_expenses = ai_cost_minor + other_expenses_minor + subscription_allocation_minor;
    let cash_flow_minor = payments_received_minor - cash_expenses;
    let profit_margin_pct = if recognized_revenue_minor > 0 {
        Some((economic_profit_minor as f64 / recognized_revenue_minor as f64) * 100.0)
    } else {
        None
    };
    let work_hours = combined_work_ms as f64 / 3_600_000.0;
    let cost_per_work_hour_minor = if work_hours > 0.0 {
        Some((total_cost_minor as f64 / work_hours).round() as i64)
    } else {
        None
    };
    let budget_remaining_minor = contracted_budget_minor - recognized_revenue_minor;
    let headroom_vs_cost = contracted_budget_minor - total_cost_minor;
    let hours_until_budget = match cost_per_work_hour_minor {
        Some(rate) if rate > 0 && headroom_vs_cost > 0 => {
            Some(headroom_vs_cost as f64 / rate as f64)
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
    fn economic_vs_cash_distinction() {
        let f = compute_project_financials(
            "USD",
            120_000,
            70_000,
            50_000,
            10_000, // labor imputed
            2_000,
            1_000,
            500,
            3_600_000,
        );
        assert_eq!(f.total_cost_minor, 13_500);
        assert_eq!(f.economic_profit_minor, 70_000 - 13_500);
        // Cash ignores imputed labor
        assert_eq!(f.cash_flow_minor, 50_000 - (2_000 + 1_000 + 500));
        assert!(f.profit_margin_pct.unwrap() > 0.0);
        assert_eq!(f.budget_remaining_minor, 120_000 - 70_000);
    }

    #[test]
    fn margin_none_when_no_revenue() {
        let f = compute_project_financials("USD", 0, 0, 0, 100, 0, 0, 0, 0);
        assert_eq!(f.profit_margin_pct, None);
        assert_eq!(f.hours_until_budget, None);
    }

    #[test]
    fn convert_and_format() {
        assert_eq!(convert_minor(100, "1.1"), Some(110));
        assert_eq!(format_money(12345, "USD"), "USD 123.45");
        assert_eq!(format_money(-50, "USD"), "-USD 0.50");
    }
}
