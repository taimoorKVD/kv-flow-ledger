# FlowLedger Financial Calculation Specification

## Units

- All monetary amounts are stored as **integer minor units** (cents for USD).
- Display formatting divides by 100 with two decimal places.
- Binary floating point is never used for stored money.

## Labor valuation

```
hourly_rate_minor = round(monthly_salary_minor / monthly_paid_hours)
task_labor_cost_minor = round((task_work_ms / 3_600_000) * hourly_rate_minor)
```

Labor valuation is an **economic** cost. It is not automatically a cash outflow.

## Project cost

```
project_cost = labor_cost + verified_or_allocated_ai_cost + other_expenses + subscription_allocation
```

Subscription allocation and metered AI charges must not double-count the same underlying spend.

## Profitability

```
economic_profit = recognized_revenue − project_cost
profit_margin_pct = economic_profit / recognized_revenue × 100   (only if revenue > 0)
cash_flow = payments_received − (ai_cost + other_expenses + subscription_allocation)
budget_remaining = contracted_budget − recognized_revenue
```

Contract budget is **not** recognized revenue.

## Forecasting

```
cost_per_work_hour = project_cost / work_hours   (if work_hours > 0)
hours_until_budget = (contracted_budget − project_cost) / cost_per_work_hour
```

Completion forecasts are withheld unless a grounded remaining-work estimate exists.

## Multi-currency

Exchange rates are stored as decimal strings with an effective date. Conversion:

```
quote_minor = round(base_minor × rate)
```

Historical calculations should use the rate effective on the transaction date.
