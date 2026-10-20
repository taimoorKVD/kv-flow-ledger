import { useEffect, useMemo, useState } from "react";
import { Plus } from "lucide-react";
import { api, type UsageRecord } from "@/lib/api";
import { compactTokens, money } from "@/lib/format";
import { FilterBar, PaginationBar, useClientPage } from "@/components/ListControls";
import { useAppStore } from "@/stores/appStore";

const PAGE_SIZE = 15;

export default function UsagePage({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const projects = useAppStore((s) => s.projects);
  const tasks = useAppStore((s) => s.tasks);
  const dashboard = useAppStore((s) => s.dashboard);
  const setToast = useAppStore((s) => s.setToast);
  const projectCcy =
    projects.find((p) => p.id === selectedProjectId)?.currency ||
    dashboard?.currency ||
    "USD";
  const [rows, setRows] = useState<UsageRecord[]>([]);
  const [open, setOpen] = useState(false);
  const [taskId, setTaskId] = useState("");
  const [tokens, setTokens] = useState("");
  const [cost, setCost] = useState("");
  const [source, setSource] = useState("Manual entry");
  const [csvText, setCsvText] = useState("");
  const [search, setSearch] = useState("");
  const [sourceFilter, setSourceFilter] = useState("all");
  const [evidenceFilter, setEvidenceFilter] = useState("all");
  const [page, setPage] = useState(1);

  useEffect(() => {
    void api.listUsage(selectedProjectId ?? undefined).then(setRows).catch(() => setRows([]));
  }, [selectedProjectId]);

  useEffect(() => {
    setPage(1);
  }, [search, sourceFilter, evidenceFilter, selectedProjectId]);

  const sources = useMemo(
    () => Array.from(new Set(rows.map((r) => r.billing_source))).sort(),
    [rows]
  );
  const evidences = useMemo(
    () => Array.from(new Set(rows.map((r) => r.verification_status))).sort(),
    [rows]
  );

  const filtered = useMemo(() => {
    return rows.filter((u) => {
      if (sourceFilter !== "all" && u.billing_source !== sourceFilter) return false;
      if (evidenceFilter !== "all" && u.verification_status !== evidenceFilter) return false;
      if (search.trim()) {
        const q = search.trim().toLowerCase();
        const hay = [u.model, u.billing_source, u.verification_status, u.notes, u.occurred_at]
          .filter(Boolean)
          .join(" ")
          .toLowerCase();
        if (!hay.includes(q)) return false;
      }
      return true;
    });
  }, [rows, sourceFilter, evidenceFilter, search]);

  const { slice, total, totalPages, page: safePage } = useClientPage(
    filtered,
    page,
    PAGE_SIZE
  );

  async function addManual() {
    const tokenNum = tokens.trim() === "" ? null : Number(tokens);
    const costNum = cost.trim() === "" ? null : Math.round(Number(cost) * 100);
    if (tokenNum != null && (!Number.isFinite(tokenNum) || tokenNum < 0)) {
      return setToast("Invalid tokens");
    }
    await api.addUsage({
      task_id: taskId || null,
      project_id: selectedProjectId,
      model: null,
      occurred_at: new Date().toISOString(),
      input_tokens: null,
      output_tokens: null,
      cached_tokens: null,
      total_tokens: tokenNum,
      reported_cost_minor: costNum,
      currency: projectCcy,
      billing_source: source,
      verification_status: tokenNum == null ? "unavailable" : "manual",
      notes: "User supplied",
      external_key: null,
    });
    setOpen(false);
    setToast("Usage record added");
    setRows(await api.listUsage(selectedProjectId ?? undefined));
    await onRefresh();
  }

  async function importCsv() {
    if (!csvText.trim()) return setToast("Paste CSV first");
    const res = await api.importUsageCsv("paste.csv", csvText);
    setToast(
      res.skipped_duplicate_import
        ? "Identical import skipped (idempotent)"
        : `Imported ${res.inserted} rows`
    );
    setRows(await api.listUsage(selectedProjectId ?? undefined));
    await onRefresh();
  }

  return (
    <section className="space-y-4">
      <div className="grid gap-3 md:grid-cols-3">
        <div className="panel p-5">
          <span className="label">All logged tokens</span>
          <div className="kpi mt-2">
            {compactTokens(dashboard?.ai_tokens, Boolean(dashboard?.ai_tokens_available))}
          </div>
          <p className="tiny mt-2">Missing data shown as Unavailable — never fabricated</p>
        </div>
        <div className="panel p-5">
          <span className="label">AI allocation</span>
          <div className="kpi mt-2">{money(dashboard?.ai_minor ?? 0, projectCcy)}</div>
          <p className="tiny mt-2">Avoid double-counting subscription fees</p>
        </div>
        <div className="panel p-5">
          <span className="label">Data coverage</span>
          <div className="mt-3 text-xl font-bold amber">Source-labeled</div>
          <p className="tiny mt-2">Do not interpret estimates as verified Cursor billing</p>
        </div>
      </div>

      <div className="panel p-5">
        <div className="flex flex-col justify-between gap-3 sm:flex-row sm:items-center">
          <div>
            <h2 className="text-lg font-bold">Usage ledger</h2>
            <p className="tiny mt-1">Project-scoped records with source, cost basis and task assignment.</p>
          </div>
          <button type="button" className="btn" onClick={() => setOpen(true)}>
            <Plus className="h-4 w-4" /> Import / add usage
          </button>
        </div>
        <FilterBar>
          <div className="min-w-[160px] flex-1">
            <label className="label">Search</label>
            <input
              className="field mt-1"
              value={search}
              placeholder="Model, source, notes…"
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          <div>
            <label className="label">Source</label>
            <select
              className="field mt-1"
              value={sourceFilter}
              onChange={(e) => setSourceFilter(e.target.value)}
            >
              <option value="all">All sources</option>
              {sources.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="label">Evidence</label>
            <select
              className="field mt-1"
              value={evidenceFilter}
              onChange={(e) => setEvidenceFilter(e.target.value)}
            >
              <option value="all">All evidence</option>
              {evidences.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </select>
          </div>
        </FilterBar>
        <div className="overflow-x-auto">
          <table className="table">
            <thead>
              <tr>
                <th>When</th>
                <th>Tokens</th>
                <th>Cost</th>
                <th>Source</th>
                <th>Evidence</th>
              </tr>
            </thead>
            <tbody>
              {slice.length === 0 && (
                <tr>
                  <td colSpan={5}>No usage records match the current filters</td>
                </tr>
              )}
              {slice.map((u) => (
                <tr key={u.id}>
                  <td>{u.occurred_at}</td>
                  <td>
                    {u.total_tokens == null
                      ? "Unavailable"
                      : u.total_tokens.toLocaleString()}
                  </td>
                  <td>
                    {u.reported_cost_minor == null
                      ? "Unavailable"
                      : money(u.reported_cost_minor, projectCcy)}
                  </td>
                  <td>{u.billing_source}</td>
                  <td>
                    <span className="tag">{u.verification_status}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <PaginationBar
          page={safePage}
          totalPages={totalPages}
          total={total}
          pageSize={PAGE_SIZE}
          onPage={setPage}
        />
      </div>

      <div className="panel p-5">
        <h2 className="font-bold">CSV import (Cursor Usage export)</h2>
        <p className="mt-2 text-sm text-muted">
          Paste a CSV exported from Cursor Dashboard → Usage. Imports are idempotent by content hash.
          Enterprise Admin APIs are not used.
        </p>
        <textarea
          className="field mt-3 min-h-[120px] font-mono text-xs"
          value={csvText}
          onChange={(e) => setCsvText(e.target.value)}
          placeholder={"id,model,tokens,cost\n..."}
        />
        <button type="button" className="btn mt-3" onClick={() => void importCsv()}>
          Import CSV
        </button>
      </div>

      {open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#030913c9] p-4">
          <div className="panel w-full max-w-lg space-y-4 p-6">
            <h2 className="text-lg font-bold">Add usage record</h2>
            <div>
              <label className="label">Task</label>
              <select className="field mt-1" value={taskId} onChange={(e) => setTaskId(e.target.value)}>
                <option value="">Unassigned</option>
                {tasks.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.title}
                  </option>
                ))}
              </select>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="label">Tokens (leave blank = Unavailable)</label>
                <input className="field mt-1" value={tokens} onChange={(e) => setTokens(e.target.value)} />
              </div>
              <div>
                <label className="label">Allocated cost ({projectCcy}, optional)</label>
                <input className="field mt-1" value={cost} onChange={(e) => setCost(e.target.value)} />
              </div>
            </div>
            <div>
              <label className="label">Record source</label>
              <select className="field mt-1" value={source} onChange={(e) => setSource(e.target.value)}>
                <option>Manual entry</option>
                <option>Cursor usage export</option>
                <option>Other usage export</option>
              </select>
            </div>
            <div className="flex justify-end gap-2">
              <button type="button" className="btn-ghost" onClick={() => setOpen(false)}>
                Cancel
              </button>
              <button type="button" className="btn" onClick={() => void addManual()}>
                Save
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
