import { useEffect, useState, type ReactNode } from "react";
import { FolderOpen, Pencil, Plus, Trash2 } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, type WorkspacePathCheck } from "@/lib/api";
import { dur, money } from "@/lib/format";
import { FilterBar, PaginationBar, useClientPage } from "@/components/ListControls";
import { useAppStore } from "@/stores/appStore";

const TASK_PAGE_SIZE = 10;

export default function ProjectsPage({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const projects = useAppStore((s) => s.projects);
  const tasks = useAppStore((s) => s.tasks);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const setSelectedProjectId = useAppStore((s) => s.setSelectedProjectId);
  const setToast = useAppStore((s) => s.setToast);

  const [showProject, setShowProject] = useState(false);
  const [editingProject, setEditingProject] = useState(false);
  const [pName, setPName] = useState("");
  const [pClient, setPClient] = useState("");
  const [pWorkspace, setPWorkspace] = useState("");
  const [pWorkspaceCheck, setPWorkspaceCheck] = useState<WorkspacePathCheck | null>(null);
  const [pBudget, setPBudget] = useState("0");
  const [pCurrency, setPCurrency] = useState("USD");
  const [budget, setBudget] = useState("");
  const [revenue, setRevenue] = useState("");
  const [received, setReceived] = useState("");
  const [other, setOther] = useState("");
  const [laborRate, setLaborRate] = useState("");
  const [computedLabor, setComputedLabor] = useState(0);
  const [projCurrency, setProjCurrency] = useState("USD");
  const [currencies, setCurrencies] = useState<
    Array<{ code: string; name: string; symbol: string }>
  >([]);
  const [editingTaskId, setEditingTaskId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [taskSearch, setTaskSearch] = useState("");
  const [taskStatus, setTaskStatus] = useState("all");
  const [taskPage, setTaskPage] = useState(1);

  const project = projects.find((p) => p.id === selectedProjectId);
  const ccy = project?.currency || projCurrency || "USD";

  const filteredTasks = tasks.filter((t) => {
    if (taskStatus !== "all" && t.status !== taskStatus) return false;
    if (taskSearch.trim()) {
      const q = taskSearch.trim().toLowerCase();
      if (!t.title.toLowerCase().includes(q)) return false;
    }
    return true;
  });
  const taskStatuses = Array.from(new Set(tasks.map((t) => t.status))).sort();
  const { slice: pagedTasks, total: taskTotal, totalPages: taskPages, page: safeTaskPage } =
    useClientPage(filteredTasks, taskPage, TASK_PAGE_SIZE);

  useEffect(() => {
    setTaskPage(1);
  }, [taskSearch, taskStatus, selectedProjectId]);

  useEffect(() => {
    void api.listCurrencies().then(setCurrencies).catch(() => {
      setCurrencies([
        { code: "USD", name: "US Dollar", symbol: "$" },
        { code: "PKR", name: "Pakistani Rupee", symbol: "Rs" },
      ]);
    });
  }, []);

  useEffect(() => {
    if (!project || !selectedProjectId) {
      setBudget("");
      setRevenue("");
      setReceived("");
      setOther("");
      setLaborRate("");
      setComputedLabor(0);
      return;
    }
    setProjCurrency(project.currency || "USD");
    setBudget(String(project.contracted_budget_minor / 100));
    setLaborRate(
      project.labor_rate_override_minor != null && project.labor_rate_override_minor > 0
        ? String(project.labor_rate_override_minor / 100)
        : ""
    );
    void api
      .getProjectFinancials(selectedProjectId)
      .then((fin) => {
        setBudget(String(fin.contracted_budget_minor / 100));
        setRevenue(String(fin.recognized_revenue_minor / 100));
        setReceived(String(fin.payments_received_minor / 100));
        setOther(
          fin.other_expenses_minor > 0 ? String(fin.other_expenses_minor / 100) : ""
        );
        setComputedLabor(fin.labor_cost_minor);
      })
      .catch(() => {
        setRevenue("");
        setReceived("");
        setOther("");
        setComputedLabor(0);
      });
  }, [project, selectedProjectId]);

  useEffect(() => {
    if (!pWorkspace.trim()) {
      setPWorkspaceCheck(null);
      return;
    }
    const t = window.setTimeout(() => {
      void api
        .validateWorkspacePath(pWorkspace)
        .then(setPWorkspaceCheck)
        .catch(() =>
          setPWorkspaceCheck({
            valid: false,
            exists: false,
            is_dir: false,
            normalized: pWorkspace,
            detail: "Could not validate path",
          })
        );
    }, 350);
    return () => window.clearTimeout(t);
  }, [pWorkspace]);

  function openCreateModal() {
    setEditingProject(false);
    setPName("");
    setPClient("");
    setPWorkspace("");
    setPWorkspaceCheck(null);
    setPBudget("0");
    setPCurrency("USD");
    setShowProject(true);
  }

  function openEditModal() {
    if (!project) return;
    setEditingProject(true);
    setPName(project.name);
    setPClient(project.client ?? "");
    setPWorkspace(project.workspaces[0] ?? "");
    setPBudget(String(project.contracted_budget_minor / 100));
    setPCurrency(project.currency || "USD");
    setShowProject(true);
  }

  async function pickWorkspaceFolder() {
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: "Select project workspace folder",
      });
      if (typeof selected === "string" && selected) {
        setPWorkspace(selected);
      }
    } catch {
      setToast("Folder picker unavailable — paste an absolute path");
    }
  }

  async function saveProject() {
    if (!pName.trim()) return setToast("Enter project name");
    const input = {
      name: pName.trim(),
      client: pClient.trim() || null,
      description: null,
      git_repository: null,
      billing_model: project?.billing_model || "fixed_price",
      contracted_budget_minor: Math.round(Number(pBudget || 0) * 100),
      currency: pCurrency,
      workspaces: pWorkspace.trim() ? [pWorkspace.trim()] : [],
    };
    if (editingProject) {
      if (!selectedProjectId) return;
      await api.updateProject(selectedProjectId, input);
      setShowProject(false);
      setToast("Project updated");
    } else {
      const created = await api.createProject(input);
      setSelectedProjectId(created.id);
      setShowProject(false);
      setToast(`Project created (${pCurrency})`);
    }
    await onRefresh();
  }

  async function deleteSelectedProject() {
    if (!project || !selectedProjectId) return;
    const ok = window.confirm(
      `Delete project “${project.name}”? Tasks and workspace mappings for this project will be removed. Activity history stays but becomes unassigned.`
    );
    if (!ok) return;
    setDeleting(true);
    try {
      await api.deleteProject(selectedProjectId);
      setSelectedProjectId(null);
      setToast("Project deleted");
      await onRefresh();
    } finally {
      setDeleting(false);
    }
  }

  async function saveFinances() {
    if (!selectedProjectId) return;
    const laborOverride =
      laborRate.trim() === "" || Number(laborRate) <= 0
        ? null
        : Math.round(Number(laborRate) * 100);
    await api.updateProjectFinances(selectedProjectId, {
      contracted_budget_minor: Math.round(Number(budget || 0) * 100),
      recognized_revenue_minor: Math.round(Number(revenue || 0) * 100),
      payments_received_minor: Math.round(Number(received || 0) * 100),
      other_expenses_minor: Math.round(Number(other || 0) * 100),
      labor_rate_override_minor: laborOverride,
      currency: projCurrency,
    });
    setToast(`Project finances saved in ${projCurrency}`);
    await onRefresh();
    const fin = await api.getProjectFinancials(selectedProjectId);
    setComputedLabor(fin.labor_cost_minor);
  }

  async function saveTaskTitle(taskId: string) {
    const title = editingTitle.trim();
    if (!title) {
      setToast("Enter a task title");
      return;
    }
    await api.updateTaskTitle(taskId, title);
    setEditingTaskId(null);
    setEditingTitle("");
    setToast("Task renamed");
    await onRefresh();
  }

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap gap-2">
        <button type="button" className="btn" onClick={openCreateModal}>
          <Plus className="h-4 w-4" /> New project
        </button>
        <button
          type="button"
          className="btn-ghost"
          disabled={!project}
          onClick={openEditModal}
        >
          <Pencil className="h-4 w-4" /> Edit project
        </button>
        <button
          type="button"
          className="btn-ghost"
          disabled={!project || deleting}
          onClick={() => void deleteSelectedProject()}
        >
          <Trash2 className="h-4 w-4" /> Delete project
        </button>
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <div className="panel p-5 lg:col-span-2">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-lg font-bold">Tasks & conversations</h2>
              <p className="tiny mt-1">
                Tasks are created when you start or continue a Cursor chat in a mapped workspace.
                Click a title to rename.
              </p>
            </div>
          </div>
          <FilterBar>
            <div className="min-w-[160px] flex-1">
              <label className="label">Search tasks</label>
              <input
                className="field mt-1"
                value={taskSearch}
                placeholder="Task title"
                onChange={(e) => setTaskSearch(e.target.value)}
              />
            </div>
            <div>
              <label className="label">Status</label>
              <select
                className="field mt-1"
                value={taskStatus}
                onChange={(e) => setTaskStatus(e.target.value)}
              >
                <option value="all">All statuses</option>
                {taskStatuses.map((s) => (
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
                  <th>Task</th>
                  <th>Status</th>
                  <th>Time</th>
                  <th>Agent</th>
                </tr>
              </thead>
              <tbody>
                {pagedTasks.length === 0 && (
                  <tr>
                    <td colSpan={4}>
                      {!selectedProjectId
                        ? "Select a project."
                        : "No tasks yet — open a Cursor chat in a mapped workspace."}
                    </td>
                  </tr>
                )}
                {pagedTasks.map((t) => (
                  <tr key={t.id}>
                    <td>
                      {editingTaskId === t.id ? (
                        <input
                          className="field"
                          value={editingTitle}
                          autoFocus
                          onChange={(e) => setEditingTitle(e.target.value)}
                          onBlur={() => void saveTaskTitle(t.id)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") {
                              e.preventDefault();
                              void saveTaskTitle(t.id);
                            }
                            if (e.key === "Escape") {
                              setEditingTaskId(null);
                              setEditingTitle("");
                            }
                          }}
                        />
                      ) : (
                        <button
                          type="button"
                          className="font-semibold text-left hover:underline"
                          onClick={() => {
                            setEditingTaskId(t.id);
                            setEditingTitle(t.title);
                          }}
                        >
                          {t.title}
                        </button>
                      )}
                    </td>
                    <td>
                      <span className="tag">{t.status}</span>
                    </td>
                    <td>{dur(t.combined_work_ms)}</td>
                    <td>{dur(t.agent_ms)}</td>
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
          <h2 className="text-lg font-bold">Project finances</h2>
          <p className="tiny mt-1">
            Total payment, received cash, and labor are project-specific. Other cost is optional.
          </p>
          {!project ? (
            <p className="mt-4 text-sm text-muted">Select or create a project.</p>
          ) : (
            <div className="mt-5 space-y-4">
              <div>
                <div className="label">Project</div>
                <div className="mt-1 font-semibold">{project.name}</div>
                {project.client && <div className="tiny mt-1">Client: {project.client}</div>}
                <div className="tiny mt-1">
                  Workspaces:{" "}
                  {project.workspaces.length ? project.workspaces.join(", ") : "None mapped"}
                </div>
              </div>
              <div>
                <label className="label">Project currency</label>
                <select
                  className="field mt-1"
                  value={projCurrency}
                  onChange={(e) => setProjCurrency(e.target.value)}
                >
                  {currencies.map((c) => (
                    <option key={c.code} value={c.code}>
                      {c.code} — {c.name}
                    </option>
                  ))}
                </select>
              </div>
              <Field
                label={`Total payment / contracted (${ccy})`}
                value={budget}
                onChange={setBudget}
                placeholder="0"
              />
              <Field
                label={`Recognized revenue (${ccy})`}
                value={revenue}
                onChange={setRevenue}
                placeholder="0"
              />
              <Field
                label={`Received payment (${ccy})`}
                value={received}
                onChange={setReceived}
                placeholder="0"
              />
              <div>
                <label className="label">
                  Labor hourly rate override ({ccy}, optional)
                </label>
                <input
                  className="field mt-1"
                  type="number"
                  min={0}
                  step="0.01"
                  value={laborRate}
                  placeholder="Use Settings salary rate"
                  onChange={(e) => setLaborRate(e.target.value)}
                />
                <p className="tiny mt-1">
                  Computed labor for tracked time:{" "}
                  <strong>{money(computedLabor, ccy)}</strong>
                </p>
              </div>
              <Field
                label={`Other expenses (${ccy}, optional)`}
                value={other}
                onChange={setOther}
                placeholder="Leave blank if none"
              />
              <button type="button" className="btn w-full" onClick={() => void saveFinances()}>
                Save project finances
              </button>
              <p className="tiny">
                All commercial figures for this project are stored and calculated in {ccy}.
              </p>
            </div>
          )}
        </div>
      </div>

      <div className="panel p-5">
        <h2 className="font-bold">Project attribution policy</h2>
        <p className="mt-2 text-sm text-muted">
          Review is scoped to the selected project. Assigning maps the workspace folder to this
          project and creates a General task when needed so future Cursor chats attribute
          automatically.
        </p>
        <UnassignedReview onRefresh={onRefresh} />
      </div>

      {showProject && (
        <Modal
          title={editingProject ? "Edit project" : "Create project"}
          onClose={() => setShowProject(false)}
          onSave={() => void saveProject()}
          saveLabel={editingProject ? "Save changes" : "Create"}
        >
          <Field label="Name" value={pName} onChange={setPName} />
          <Field label="Client" value={pClient} onChange={setPClient} />
          <div>
            <label className="label">Workspace directory</label>
            <div className="mt-1 flex gap-2">
              <input
                className={`field flex-1 ${
                  pWorkspaceCheck?.valid
                    ? "field-ok"
                    : pWorkspace.trim() && pWorkspaceCheck && !pWorkspaceCheck.valid
                      ? "field-bad"
                      : ""
                }`}
                value={pWorkspace}
                placeholder="Choose a folder or paste absolute path"
                onChange={(e) => setPWorkspace(e.target.value)}
              />
              <button type="button" className="btn-ghost shrink-0" onClick={() => void pickWorkspaceFolder()}>
                <FolderOpen className="h-4 w-4" /> Browse
              </button>
            </div>
            {pWorkspace.trim() && pWorkspaceCheck && (
              <p className={`tiny mt-1 ${pWorkspaceCheck.valid ? "path-ok" : "path-bad"}`}>
                {pWorkspaceCheck.valid ? "✓ " : "✗ "}
                {pWorkspaceCheck.detail}
                {pWorkspaceCheck.normalized
                  ? ` · ${pWorkspaceCheck.normalized}`
                  : ""}
              </p>
            )}
          </div>
          <div>
            <label className="label">Currency</label>
            <select
              className="field mt-1"
              value={pCurrency}
              onChange={(e) => setPCurrency(e.target.value)}
            >
              {currencies.map((c) => (
                <option key={c.code} value={c.code}>
                  {c.code} — {c.name}
                </option>
              ))}
            </select>
          </div>
          <Field label={`Total payment (${pCurrency})`} value={pBudget} onChange={setPBudget} />
        </Modal>
      )}
    </section>
  );
}

function UnassignedReview({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const tasks = useAppStore((s) => s.tasks);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const setToast = useAppStore((s) => s.setToast);

  const [items, setItems] = useState<Array<Record<string, string>>>([]);
  const [reasons, setReasons] = useState<string[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize] = useState(10);
  const [reason, setReason] = useState("all");
  const [conversation, setConversation] = useState("all");
  const [search, setSearch] = useState("");
  const [searchInput, setSearchInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [assigningId, setAssigningId] = useState<string | null>(null);

  async function load(nextPage = page) {
    if (!selectedProjectId) {
      setItems([]);
      setTotal(0);
      setReasons([]);
      return;
    }
    setLoading(true);
    try {
      const res = await api.listUnassignedPage({
        projectId: selectedProjectId,
        reason: reason === "all" ? null : reason,
        search: search.trim() || null,
        conversation: conversation === "all" ? null : conversation,
        page: nextPage,
        pageSize,
      });
      setItems(res.items);
      setTotal(res.total);
      setPage(res.page);
      setReasons(res.reasons ?? []);
    } catch {
      setItems([]);
      setTotal(0);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load(1);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- reload when filters / project change
  }, [reason, conversation, search, pageSize, selectedProjectId]);

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  async function assignToProject(itemId: string) {
    if (!selectedProjectId) return;
    setAssigningId(itemId);
    try {
      // Prefer an existing non-General task when present; otherwise backend creates General.
      const preferred =
        tasks.find((t) => t.title !== "General")?.id ?? tasks[0]?.id ?? null;
      await api.assignUnassigned(itemId, selectedProjectId, preferred);
      setToast("Assigned to current project (workspace mapped)");
      await onRefresh();
      await load(page);
    } catch (e) {
      setToast(e instanceof Error ? e.message : "Assign failed");
    } finally {
      setAssigningId(null);
    }
  }

  return (
    <div className="mt-4 space-y-3">
      <FilterBar>
        <div className="min-w-[160px] flex-1">
          <label className="label">Search</label>
          <input
            className="field mt-1"
            value={searchInput}
            placeholder="Workspace or conversation"
            onChange={(e) => setSearchInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") setSearch(searchInput.trim());
            }}
          />
        </div>
        <div>
          <label className="label">Reason</label>
          <select className="field mt-1" value={reason} onChange={(e) => setReason(e.target.value)}>
            <option value="all">All reasons</option>
            {reasons.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="label">Conversation</label>
          <select
            className="field mt-1"
            value={conversation}
            onChange={(e) => setConversation(e.target.value)}
          >
            <option value="all">All</option>
            <option value="with_conversation">With conversation</option>
            <option value="without_conversation">Without conversation</option>
          </select>
        </div>
        <button
          type="button"
          className="btn-ghost"
          onClick={() => {
            setSearch(searchInput.trim());
            void load(1);
          }}
        >
          Apply
        </button>
        <button type="button" className="btn-ghost" onClick={() => void load(page)}>
          Refresh
        </button>
        <button
          type="button"
          className="btn-ghost"
          onClick={async () => {
            const n = await api.dismissNoiseUnassigned();
            setToast(`Dismissed ${n} empty / insufficient-signal rows`);
            await onRefresh();
            await load(1);
          }}
        >
          Dismiss empty noise
        </button>
      </FilterBar>

      <p className="tiny">
        {!selectedProjectId
          ? "Select a project to review attribution for that project."
          : loading
            ? "Loading…"
            : `${total} pending for this project`}{" "}
        · page {page} of {totalPages}
      </p>

      {items.length === 0 ? (
        <p className="tiny">
          {!selectedProjectId
            ? "Choose a project above."
            : "No pending unassigned activity for this project."}
        </p>
      ) : (
        items.map((item) => (
          <div key={item.id} className="soft flex flex-wrap items-center justify-between gap-3 p-3">
            <div>
              <div className="text-sm font-semibold">{item.reason}</div>
              <div className="tiny">
                {item.workspace_path || "—"} · {item.conversation_id || "no conversation"}
              </div>
              <p className="tiny mt-1">
                Assign maps this workspace to the selected project and links the conversation.
              </p>
            </div>
            <button
              type="button"
              className="btn"
              disabled={!selectedProjectId || assigningId === item.id}
              onClick={() => void assignToProject(item.id)}
            >
              {assigningId === item.id ? "Assigning…" : "Assign to current project"}
            </button>
          </div>
        ))
      )}

      <PaginationBar
        page={page}
        totalPages={totalPages}
        total={total}
        pageSize={pageSize}
        loading={loading}
        onPage={(p) => void load(p)}
      />
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <div>
      <label className="label">{label}</label>
      <input
        className="field mt-1"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

function Modal({
  title,
  children,
  onClose,
  onSave,
  saveLabel = "Save",
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  onSave: () => void;
  saveLabel?: string;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#030913c9] p-4 backdrop-blur-sm">
      <div className="panel w-full max-w-lg p-6 shadow-2xl">
        <h2 className="text-lg font-bold">{title}</h2>
        <div className="mt-5 space-y-4">{children}</div>
        <div className="mt-6 flex justify-end gap-2">
          <button type="button" className="btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="btn" onClick={onSave}>
            {saveLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
