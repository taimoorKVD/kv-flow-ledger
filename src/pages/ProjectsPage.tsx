import { useEffect, useState, type ReactNode } from "react";
import { Pencil, Plus, Trash2 } from "lucide-react";
import { api } from "@/lib/api";
import { dur } from "@/lib/format";
import { useAppStore } from "@/stores/appStore";

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
  const [pBudget, setPBudget] = useState("0");
  const [pCurrency, setPCurrency] = useState("USD");
  const [budget, setBudget] = useState("");
  const [revenue, setRevenue] = useState("");
  const [received, setReceived] = useState("");
  const [other, setOther] = useState("");
  const [projCurrency, setProjCurrency] = useState("USD");
  const [currencies, setCurrencies] = useState<
    Array<{ code: string; name: string; symbol: string }>
  >([]);
  const [editingTaskId, setEditingTaskId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");
  const [deleting, setDeleting] = useState(false);

  const project = projects.find((p) => p.id === selectedProjectId);
  const ccy = project?.currency || projCurrency || "USD";

  useEffect(() => {
    void api.listCurrencies().then(setCurrencies).catch(() => {
      setCurrencies([
        { code: "USD", name: "US Dollar", symbol: "$" },
        { code: "PKR", name: "Pakistani Rupee", symbol: "Rs" },
      ]);
    });
  }, []);

  useEffect(() => {
    if (project) {
      setProjCurrency(project.currency || "USD");
      setBudget(String(project.contracted_budget_minor / 100));
    }
  }, [project]);

  function openCreateModal() {
    setEditingProject(false);
    setPName("");
    setPClient("");
    setPWorkspace("");
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
    await api.updateProjectFinances(selectedProjectId, {
      contracted_budget_minor: Math.round(Number(budget || 0) * 100),
      recognized_revenue_minor: Math.round(Number(revenue || 0) * 100),
      payments_received_minor: Math.round(Number(received || 0) * 100),
      other_expenses_minor: Math.round(Number(other || 0) * 100),
      currency: projCurrency,
    });
    setToast(`Project finances saved in ${projCurrency}`);
    await onRefresh();
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
          <div className="mt-4 overflow-x-auto">
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
                {tasks.length === 0 && (
                  <tr>
                    <td colSpan={4}>No tasks yet — open a Cursor chat in a mapped workspace.</td>
                  </tr>
                )}
                {tasks.map((t) => (
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
        </div>

        <div className="panel p-5">
          <h2 className="text-lg font-bold">Project details</h2>
          <p className="tiny mt-1">Enter commercial values independently from tracking.</p>
          {!project ? (
            <p className="mt-4 text-sm text-muted">Select or create a project.</p>
          ) : (
            <div className="mt-5 space-y-4">
              <div>
                <div className="label">Project</div>
                <div className="mt-1 font-semibold">{project.name}</div>
                {project.client && <div className="tiny mt-1">Client: {project.client}</div>}
                <div className="tiny mt-1">
                  Workspaces: {project.workspaces.length ? project.workspaces.join(", ") : "None mapped"}
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
                <p className="tiny mt-1">
                  Labor (from Settings salary currency) and AI costs are converted into this currency for P&amp;L.
                </p>
              </div>
              <Field label={`Contracted budget (${ccy})`} value={budget} onChange={setBudget} placeholder={String(project.contracted_budget_minor / 100)} />
              <Field label={`Recognized revenue (${ccy})`} value={revenue} onChange={setRevenue} />
              <Field label={`Payments received (${ccy})`} value={received} onChange={setReceived} />
              <Field label={`Other expenses (${ccy})`} value={other} onChange={setOther} />
              <button type="button" className="btn w-full" onClick={() => void saveFinances()}>
                Save project finances
              </button>
              <p className="tiny">
                All commercial figures for this project are stored and calculated in {ccy}. Profit uses recognized revenue, not budget.
              </p>
            </div>
          )}
        </div>
      </div>

      <div className="panel p-5">
        <h2 className="font-bold">Project attribution policy</h2>
        <p className="mt-2 text-sm text-muted">
          Map a workspace folder on a project once. Cursor chats in that folder auto-create tasks;
          workspace-only activity uses a General task. Only unknown workspaces appear below for review.
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
          <Field label="Workspace directory (absolute path)" value={pWorkspace} onChange={setPWorkspace} />
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
          <Field label={`Contracted budget (${pCurrency})`} value={pBudget} onChange={setPBudget} />
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

  return (
    <div className="mt-4 space-y-3">
      <div className="flex flex-wrap items-end gap-2">
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
      </div>

      <p className="tiny">
        {!selectedProjectId
          ? "Select a project to review its unassigned activity."
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
                Add this path as a project workspace, or assign to an existing project task.
              </p>
            </div>
            <button
              type="button"
              className="btn-ghost"
              disabled={!selectedProjectId || tasks.length === 0}
              onClick={async () => {
                if (!selectedProjectId || !tasks[0]) return;
                await api.assignUnassigned(item.id, selectedProjectId, tasks[0].id);
                setToast("Assigned to first task in project");
                await onRefresh();
                await load(page);
              }}
            >
              Assign to current project task
            </button>
          </div>
        ))
      )}

      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          className="btn-ghost"
          disabled={page <= 1 || loading}
          onClick={() => void load(page - 1)}
        >
          Previous
        </button>
        <button
          type="button"
          className="btn-ghost"
          disabled={page >= totalPages || loading}
          onClick={() => void load(page + 1)}
        >
          Next
        </button>
      </div>
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
