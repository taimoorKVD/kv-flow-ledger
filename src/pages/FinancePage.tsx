import { useEffect, useState } from "react";
import ReactECharts from "echarts-for-react";
import { api, type ProjectFinancials } from "@/lib/api";
import { money } from "@/lib/format";
import { useAppStore } from "@/stores/appStore";

export default function FinancePage() {
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const [fin, setFin] = useState<ProjectFinancials | null>(null);

  useEffect(() => {
    if (!selectedProjectId) {
      setFin(null);
      return;
    }
    void api.getProjectFinancials(selectedProjectId).then(setFin).catch(() => setFin(null));
  }, [selectedProjectId]);

  if (!selectedProjectId || !fin) {
    return <div className="panel p-8 text-sm text-muted">Select a project with financial records.</div>;
  }

  const contractProfit = fin.contract_profit_minor;
  const contractMargin = fin.contract_margin_pct ?? fin.profit_margin_pct;
  const hasContract = (fin.contracted_budget_minor ?? 0) > 0;
  const chartOption = {
    tooltip: { trigger: "item" },
    series: [
      {
        type: "pie",
        radius: ["42%", "68%"],
        label: { color: "#c0d2eb" },
        data: [
          { name: "Labor", value: fin.labor_cost_minor / 100 },
          { name: "AI", value: fin.ai_cost_minor / 100 },
          { name: "Other", value: fin.other_expenses_minor / 100 },
          { name: "Subscription alloc.", value: fin.subscription_allocation_minor / 100 },
        ].filter((d) => d.value > 0),
      },
    ],
  };

  return (
    <section className="space-y-4">
      <div className="panel p-5">
        <h2 className="text-lg font-bold">Contract profit &amp; loss</h2>
        <p className="tiny mt-1">
          Expenses (labor + Cursor AI + other) always accumulate. Profit/loss is{" "}
          <strong>contracted payment − expenses</strong> and stays N/A until total payment is set.
        </p>
        <div className="mt-5 grid gap-3 md:grid-cols-4">
          <Metric label="Contracted payment" value={money(fin.contracted_budget_minor, fin.currency)} />
          <Metric label="Total expenses" value={money(fin.total_cost_minor, fin.currency)} />
          <Metric
            label="Profit / loss"
            value={
              !hasContract || contractProfit == null
                ? "N/A"
                : money(contractProfit, fin.currency)
            }
            className={
              !hasContract || contractProfit == null
                ? undefined
                : contractProfit < 0
                  ? "red"
                  : "green"
            }
          />
          <Metric
            label="Contract margin"
            value={
              contractMargin == null
                ? "N/A"
                : `${contractMargin >= 0 ? "" : "−"}${Math.abs(contractMargin).toFixed(1)}%`
            }
            className={
              contractMargin == null ? undefined : contractMargin < 0 ? "red" : "green"
            }
          />
        </div>
      </div>

      <div className="grid gap-3 md:grid-cols-4">
        <Metric label="Labor expense" value={money(fin.labor_cost_minor, fin.currency)} />
        <Metric label="AI allocation (Cursor)" value={money(fin.ai_cost_minor, fin.currency)} />
        <Metric label="Other expenses" value={money(fin.other_expenses_minor, fin.currency)} />
        <Metric label="Received payment" value={money(fin.payments_received_minor, fin.currency)} />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <div className="panel p-5">
          <h2 className="text-lg font-bold">Expense breakdown</h2>
          <p className="tiny mt-1">All lines below reduce contract profit the same way.</p>
          <div className="mt-6 space-y-5">
            {[
              [
                "Labor cost",
                fin.labor_cost_minor,
                "#3ecf9a",
                fin.labor_of_contract_pct,
              ],
              [
                "AI allocation (Cursor)",
                fin.ai_cost_minor,
                "#5eead4",
                fin.ai_of_contract_pct,
              ],
              ["Other expenses", fin.other_expenses_minor, "#f0bc6d", null],
              [
                "Subscription allocation",
                fin.subscription_allocation_minor,
                "#2bb888",
                null,
              ],
            ].map(([n, v, c, pct]) => (
              <div key={String(n)}>
                <div className="mb-2 flex justify-between text-sm">
                  <span>
                    {n}
                    {typeof pct === "number" ? (
                      <span className="tiny ml-2">({pct.toFixed(1)}% of contract)</span>
                    ) : null}
                  </span>
                  <strong>{money(Number(v), fin.currency)}</strong>
                </div>
                <div className="progress">
                  <span
                    style={{
                      background: String(c),
                      width: `${(100 * Number(v)) / Math.max(1, fin.total_cost_minor)}%`,
                    }}
                  />
                </div>
              </div>
            ))}
          </div>
          <div className="mt-6 flex items-center justify-between border-t border-stroke pt-5">
            <span className="label">Total expenses</span>
            <strong className="text-2xl">{money(fin.total_cost_minor, fin.currency)}</strong>
          </div>
          <p className="tiny mt-3">
            Cash result (received − all expenses):{" "}
            <strong className={fin.cash_flow_minor < 0 ? "red" : "green"}>
              {money(fin.cash_flow_minor, fin.currency)}
            </strong>
          </p>
        </div>

        <div className="panel p-5">
          <h2 className="text-lg font-bold">Forecast & unit economics</h2>
          <div className="mt-5 space-y-4">
            <div className="soft p-4">
              <div className="tiny">Average cost / task-work hour</div>
              <div className="mt-1 text-2xl font-bold">
                {fin.cost_per_work_hour_minor == null
                  ? "N/A"
                  : money(fin.cost_per_work_hour_minor, fin.currency)}
              </div>
            </div>
            <div className="soft p-4">
              <div className="tiny">Contract remaining after expenses</div>
              <div
                className={`mt-1 text-2xl font-bold ${
                  fin.budget_remaining_minor == null
                    ? ""
                    : fin.budget_remaining_minor < 0
                      ? "red"
                      : "green"
                }`}
              >
                {fin.budget_remaining_minor == null
                  ? "N/A"
                  : money(fin.budget_remaining_minor, fin.currency)}
              </div>
            </div>
            <div className="soft p-4">
              <div className="tiny">Estimated hours until expenses consume contract</div>
              <div className="mt-1 text-2xl font-bold">
                {fin.hours_until_budget == null
                  ? "N/A"
                  : `${fin.hours_until_budget.toFixed(1)} h`}
              </div>
            </div>
          </div>
          <p className="tiny mt-4">
            Runway assumes the observed average spending rate (labor + AI + other) stays constant.
          </p>
          <div className="mt-4 h-[220px]">
            <ReactECharts option={chartOption} style={{ height: "100%", width: "100%" }} />
          </div>
        </div>
      </div>
    </section>
  );
}

function Metric({
  label,
  value,
  className,
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div className="panel p-5">
      <span className="label">{label}</span>
      <div className={`kpi mt-2 ${className ?? ""}`}>{value}</div>
    </div>
  );
}
