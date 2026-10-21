import { useMemo, useState, type ReactNode } from "react";
import { Clock, Cpu, Receipt, TrendingUp, RadioTower } from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { compactTokens, dur, margin, money } from "@/lib/format";
import { FilterBar, PaginationBar, useClientPage } from "@/components/ListControls";
import { api } from "@/lib/api";

const TASK_PAGE_SIZE = 8;

export default function OverviewPage({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const dashboard = useAppStore((s) => s.dashboard);
  const tasks = useAppStore((s) => s.tasks);
  const setPage = useAppStore((s) => s.setPage);
  const setToast = useAppStore((s) => s.setToast);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const [taskSearch, setTaskSearch] = useState("");
  const [taskPage, setTaskPage] = useState(1);

  const filteredTasks = useMemo(() => {
    if (!taskSearch.trim()) return tasks;
    const q = taskSearch.trim().toLowerCase();
    return tasks.filter((t) => t.title.toLowerCase().includes(q) || t.status.toLowerCase().includes(q));
  }, [tasks, taskSearch]);
  const { slice: pagedTasks, total: taskTotal, totalPages: taskPages, page: safeTaskPage } =
    useClientPage(filteredTasks, taskPage, TASK_PAGE_SIZE);

  if (!dashboard) {
    return (
      <div className="panel p-8 text-sm text-muted">
        No dashboard data yet. Create a project to begin.
      </div>
    );
  }

  const cost = Math.max(dashboard.allocated_cost_minor, 1);
  const tracking = dashboard.tracking;
  const modeLabel =
    tracking.mode === "agent"
      ? "● Agent executing"
      : tracking.mode === "editor"
        ? "● Editor active"
        : "● Paused · idle";

  async function simulate(mode: string) {
    const type =
      mode === "agent"
        ? "preToolUse"
        : mode === "editor"
          ? "afterFileEdit"
          : mode === "idle"
            ? "idle"
            : "beforeSubmitPrompt";
    await api.ingestTestEvent({
      event_type: type,
      occurred_at: new Date().toISOString(),
      workspace_roots: null,
      conversation_id: tracking.active_conversation_id ?? "local-test",
      generation_id: `gen-${Date.now()}`,
      model: null,
      model_id: null,
      agent_state: mode === "agent" ? "executing" : null,
      cursor_version: null,
      payload: { source: "ui_simulate", mode },
    });
    setToast(`Simulated ${mode} event`);
    await onRefresh();
  }

  return (
    <section className="space-y-5">
      <div className="panel flex flex-col justify-between gap-3 px-4 py-3 md:flex-row md:items-center">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <span className="pill">{modeLabel}</span>
          <span className="text-sm font-semibold">
            {tracking.active_task_id
              ? tasks.find((t) => t.id === tracking.active_task_id)?.title ?? "Active task"
              : "No linked task"}
          </span>
          <span className="tiny">
            {tracking.active_conversation_id
              ? `Conversation ${tracking.active_conversation_id}`
              : "Awaiting Cursor conversation"}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <span className="text-xs text-muted">
            Task work time{" "}
            <strong className="ml-1 text-white">{dur(dashboard.task_work_ms)}</strong>
          </span>
          <button type="button" className="btn-ghost" onClick={() => void simulate("agent")}>
            Simulate event
          </button>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Kpi label="Task work time" icon={<Clock className="h-4 w-4 text-accent" />} value={dur(dashboard.task_work_ms)} hint="Active task intervals; no double counting" />
        <Kpi
          label="AI tokens logged"
          icon={<Cpu className="h-4 w-4 text-teal-200" />}
          value={compactTokens(dashboard.ai_tokens, dashboard.ai_tokens_available)}
          hint="Verified imports only — never fabricated"
        />
        <Kpi
          label="Allocated project cost"
          icon={<Receipt className="h-4 w-4 text-amber-300" />}
          value={money(dashboard.allocated_cost_minor, dashboard.currency)}
          hint="Labor + AI allocation + other costs"
        />
        <Kpi
          label="Contract margin"
          icon={<TrendingUp className="h-4 w-4 text-accent" />}
          value={margin(dashboard.margin_pct)}
          hint="Requires contracted payment; expenses still tracked without it"
          valueClass={
            dashboard.profit_minor == null
              ? undefined
              : dashboard.profit_minor < 0
                ? "red"
                : "green"
          }
        />
      </div>

      <div className="grid gap-4 xl:grid-cols-[1.55fr_1fr]">
        <div className="panel p-5 sm:p-6">
          <div className="flex items-center justify-between gap-3">
            <div>
              <h2 className="text-lg font-bold">Project cost composition</h2>
              <p className="tiny mt-1">Current selected project · {dashboard.currency}</p>
            </div>
            <button type="button" className="btn-ghost" onClick={() => setPage("finance")}>
              Financial details
            </button>
          </div>
          <div className="mt-6 grid gap-3 sm:grid-cols-3">
            <SoftStat label="Labor allocation" value={money(dashboard.labor_minor, dashboard.currency)} />
            <SoftStat label="AI allocation" value={money(dashboard.ai_minor, dashboard.currency)} />
            <SoftStat label="Other expenses" value={money(dashboard.other_minor, dashboard.currency)} />
          </div>
          <div className="mt-6 flex h-4 overflow-hidden rounded-full bg-[var(--progress-track)]">
            <span style={{ background: "#3ecf9a", width: `${(100 * dashboard.labor_minor) / cost}%` }} />
            <span style={{ background: "#5eead4", width: `${(100 * dashboard.ai_minor) / cost}%` }} />
            <span style={{ background: "#f0bc6d", width: `${(100 * dashboard.other_minor) / cost}%` }} />
          </div>
          <div className="mt-7 grid grid-cols-2 gap-4 border-t border-stroke pt-5">
            <div>
              <div className="tiny">Total expenses (labor + AI + other)</div>
              <div className="mt-1 text-xl font-bold">
                {money(dashboard.allocated_cost_minor, dashboard.currency)}
              </div>
            </div>
            <div>
              <div className="tiny">Contract profit / loss</div>
              <div
                className={`mt-1 text-xl font-bold ${
                  dashboard.profit_minor == null
                    ? ""
                    : dashboard.profit_minor < 0
                      ? "red"
                      : "green"
                }`}
              >
                {dashboard.profit_minor == null
                  ? "N/A"
                  : money(dashboard.profit_minor, dashboard.currency)}
              </div>
              {dashboard.profit_minor == null && (
                <p className="tiny mt-1">Enter total payment on the project to compute P/L</p>
              )}
            </div>
          </div>
        </div>

        <div className="panel p-5 sm:p-6">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-lg font-bold">Autonomous tracking</h2>
              <p className="tiny mt-1">Cursor task lifecycle</p>
            </div>
            <RadioTower className="h-5 w-5 text-accent" />
          </div>
          <div className="mt-5 space-y-4">
            <div className="soft p-4">
              <div className="text-xs text-muted">Current signal</div>
              <div className="mt-2 font-bold">{modeLabel.slice(2)}</div>
              <p className="tiny mt-2">
                {tracking.hooks_installed
                  ? "Hooks installed — waiting for Cursor events."
                  : "Install hooks in Settings to receive real Cursor events."}
              </p>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <button type="button" className="btn-ghost" onClick={() => void simulate("editor")}>Editor active</button>
              <button type="button" className="btn-ghost" onClick={() => void simulate("agent")}>Agent working</button>
              <button type="button" className="btn-ghost" onClick={() => void simulate("idle")}>Idle / paused</button>
              <button type="button" className="btn-ghost" onClick={() => void simulate("resume")}>Resume task</button>
            </div>
            {dashboard.unassigned_count > 0 && (
              <p className="tiny text-amber-300">
                {dashboard.unassigned_count} unassigned activit{dashboard.unassigned_count === 1 ? "y" : "ies"} need review
                {selectedProjectId ? "" : ""}.
              </p>
            )}
          </div>
        </div>
      </div>

      <div className="grid gap-4 xl:grid-cols-[1.4fr_1fr]">
        <div className="panel p-5">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="font-bold">Tracked tasks</h2>
            <button type="button" className="text-xs font-semibold text-accent" onClick={() => setPage("projects")}>
              View all →
            </button>
          </div>
          <FilterBar>
            <div className="min-w-[140px] flex-1">
              <label className="label">Filter</label>
              <input
                className="field mt-1"
                value={taskSearch}
                placeholder="Task or status"
                onChange={(e) => {
                  setTaskSearch(e.target.value);
                  setTaskPage(1);
                }}
              />
            </div>
          </FilterBar>
          <div className="overflow-x-auto">
            <table className="table">
              <thead>
                <tr>
                  <th>Task</th>
                  <th>Work time</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {pagedTasks.length === 0 && (
                  <tr>
                    <td colSpan={3}>No tasks yet</td>
                  </tr>
                )}
                {pagedTasks.map((t) => (
                  <tr key={t.id}>
                    <td>
                      <div className="font-semibold">{t.title}</div>
                    </td>
                    <td>{dur(t.combined_work_ms)}</td>
                    <td>
                      <span className="tag">{t.status}</span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <PaginationBar
            page={safeTaskPage}
            totalPages={taskPages}
            total={taskTotal}
            pageSize={TASK_PAGE_SIZE}
            onPage={setTaskPage}
          />
        </div>
        <div className="panel p-5">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="font-bold">Connection</h2>
            <button type="button" className="text-xs font-semibold text-accent" onClick={() => setPage("settings")}>
              Settings →
            </button>
          </div>
          <div className="space-y-3 text-sm">
            <Row k="Ingest" v={tracking.ingest_listening ? `127.0.0.1:${tracking.ingest_port}` : "Offline"} />
            <div className="flex justify-between gap-3 border-b border-[#253247] pb-2">
              <span className="text-muted">Hooks</span>
              <span className={`status-badge ${tracking.hooks_installed ? "ok" : "warn"}`}>
                {tracking.hooks_installed ? "Installed" : "Not installed"}
              </span>
            </div>
            <Row k="Active projects" v={String(dashboard.active_projects)} />
            <Row k="Active tasks" v={String(dashboard.active_tasks)} />
            <Row k="Last event" v={tracking.last_event_at ?? "—"} />
          </div>
        </div>
      </div>
    </section>
  );
}

function Kpi({
  label,
  icon,
  value,
  hint,
  valueClass,
}: {
  label: string;
  icon: ReactNode;
  value: string;
  hint: string;
  valueClass?: string;
}) {
  return (
    <div className="panel p-5">
      <div className="flex items-center justify-between">
        <span className="label">{label}</span>
        {icon}
      </div>
      <div className={`kpi mt-3 ${valueClass ?? ""}`}>{value}</div>
      <p className="tiny mt-2">{hint}</p>
    </div>
  );
}

function SoftStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="soft p-4">
      <div className="tiny">{label}</div>
      <div className="mt-2 text-2xl font-bold">{value}</div>
    </div>
  );
}

function Row({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex justify-between gap-3 border-b border-[#253247] pb-2">
      <span className="text-muted">{k}</span>
      <span className="font-semibold">{v}</span>
    </div>
  );
}
