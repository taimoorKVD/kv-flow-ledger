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
      <div className="grid gap-3 md:grid-cols-4">
        <Metric label="Contracted value" value={money(fin.contracted_budget_minor, fin.currency)} />
        <Metric label="Recognized revenue" value={money(fin.recognized_revenue_minor, fin.currency)} />
        <Metric label="Received cash" value={money(fin.payments_received_minor, fin.currency)} />
        <Metric
          label="Economic profit / loss"
          value={money(fin.economic_profit_minor, fin.currency)}
          className={fin.economic_profit_minor < 0 ? "red" : "green"}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <div className="panel p-5">
          <h2 className="text-lg font-bold">Cost attribution</h2>
          <div className="mt-6 space-y-5">
            {[
              ["Labor valuation", fin.labor_cost_minor, "#3ecf9a"],
              ["AI allocation", fin.ai_cost_minor, "#5eead4"],
              ["Other expenses", fin.other_expenses_minor, "#f0bc6d"],
              ["Subscription allocation", fin.subscription_allocation_minor, "#2bb888"],
            ].map(([n, v, c]) => (
              <div key={String(n)}>
                <div className="mb-2 flex justify-between text-sm">
                  <span>{n}</span>
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
            <span className="label">Total allocated cost</span>
            <strong className="text-2xl">{money(fin.total_cost_minor, fin.currency)}</strong>
          </div>
          <p className="tiny mt-3">
            Cash flow (receipts − cash expenses, excluding imputed labor):{" "}
            <strong>{money(fin.cash_flow_minor, fin.currency)}</strong>
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
              <div className="tiny">Budget remaining vs recognized revenue</div>
              <div className="mt-1 text-2xl font-bold">
                {money(fin.budget_remaining_minor, fin.currency)}
              </div>
            </div>
            <div className="soft p-4">
              <div className="tiny">Estimated hours until allocated cost reaches budget</div>
              <div className="mt-1 text-2xl font-bold">
                {fin.hours_until_budget == null
                  ? "N/A"
                  : `${fin.hours_until_budget.toFixed(1)} h`}
              </div>
            </div>
          </div>
          <p className="tiny mt-4">
            Runway assumes observed average spending rate remains constant; it is not a completion
            forecast. Completion ETA is withheld without a grounded remaining-work estimate.
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
