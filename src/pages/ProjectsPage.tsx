import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  ArrowLeft,
  Check,
  FolderOpen,
  FolderSync,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  Trash2,
  Unlink,
  X,
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { api, type ChildDirectory, type WorkspacePathCheck } from "@/lib/api";
import { dur, money } from "@/lib/format";
import { FilterBar, PaginationBar, useClientPage } from "@/components/ListControls";
import { useAppStore } from "@/stores/appStore";

function pathKey(path: string) {
  return path.trim().replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
}

function minorToInput(minor: number) {
  if (!Number.isFinite(minor)) return "0";
  const n = minor / 100;
  return Number.isInteger(n) ? String(n) : String(n);
}

/** Parse a money field into minor units; null if invalid. */
function parseMoneyToMinor(raw: string): number | null {
  const t = raw.trim();
  if (t === "") return 0;
  const n = Number(t);
  if (!Number.isFinite(n) || n < 0) return null;
  return Math.round(n * 100);
}

const TASK_PAGE_SIZE = 10;

export default function ProjectsPage({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const projects = useAppStore((s) => s.projects);
  const tasks = useAppStore((s) => s.tasks);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const setSelectedProjectId = useAppStore((s) => s.setSelectedProjectId);
  const projectsView = useAppStore((s) => s.projectsView);
  const setProjectsView = useAppStore((s) => s.setProjectsView);
  const projectsCreateRequest = useAppStore((s) => s.projectsCreateRequest);
  const setToast = useAppStore((s) => s.setToast);

  const [showProject, setShowProject] = useState(false);
  const [showSyncLocal, setShowSyncLocal] = useState(false);
  const [editingProject, setEditingProject] = useState(false);
  const [librarySearch, setLibrarySearch] = useState("");
  const [clearingWorkspaces, setClearingWorkspaces] = useState(false);
  const [syncingTasks, setSyncingTasks] = useState(false);
  /** Prevents the 5s project refresh from wiping in-progress finance edits. */
  const financesDirtyRef = useRef(false);
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

  function applyFinancialsForm(fin: {
    contracted_budget_minor: number;
    recognized_revenue_minor: number;
    payments_received_minor: number;
    other_expenses_minor: number;
    labor_cost_minor: number;
    currency: string;
  }, laborOverrideMinor?: number | null) {
    setProjCurrency(fin.currency || "USD");
    setBudget(minorToInput(fin.contracted_budget_minor));
    setRevenue(minorToInput(fin.recognized_revenue_minor));
    setReceived(minorToInput(fin.payments_received_minor));
    setOther(
      fin.other_expenses_minor > 0 ? minorToInput(fin.other_expenses_minor) : ""
    );
    setComputedLabor(fin.labor_cost_minor);
    if (laborOverrideMinor != null && laborOverrideMinor > 0) {
      setLaborRate(minorToInput(laborOverrideMinor));
    } else if (laborOverrideMinor === null) {
      setLaborRate("");
    }
  }

  useEffect(() => {
    financesDirtyRef.current = false;
    if (!selectedProjectId) {
      setBudget("");
      setRevenue("");
      setReceived("");
      setOther("");
      setLaborRate("");
      setComputedLabor(0);
      return;
    }
    const p = projects.find((x) => x.id === selectedProjectId);
    if (p) {
      setProjCurrency(p.currency || "USD");
      setBudget(minorToInput(p.contracted_budget_minor));
      setLaborRate(
        p.labor_rate_override_minor != null && p.labor_rate_override_minor > 0
          ? minorToInput(p.labor_rate_override_minor)
          : ""
      );
    }
    const projectId = selectedProjectId;
    void api
      .getProjectFinancials(projectId)
      .then((fin) => {
        // Ignore late responses if the user already started editing or switched project.
        if (financesDirtyRef.current) return;
        if (useAppStore.getState().selectedProjectId !== projectId) return;
        const current = useAppStore.getState().projects.find((x) => x.id === projectId);
        applyFinancialsForm(
          fin,
          current?.labor_rate_override_minor ?? null
        );
      })
      .catch(() => {
        if (financesDirtyRef.current) return;
        setRevenue("");
        setReceived("");
        setOther("");
        setComputedLabor(0);
      });
    // Only reload when the selected project changes — not on every 5s projects refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedProjectId]);

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

  useEffect(() => {
    if (projectsCreateRequest > 0) {
      setProjectsView("library");
      openCreateModal();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- open create when header requests it
  }, [projectsCreateRequest]);

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

  function openProjectDetail(id: string) {
    setSelectedProjectId(id);
    setProjectsView("detail");
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
      setShowProject(false);
      setToast(`Project created (${pCurrency})`);
      await onRefresh();
      openProjectDetail(created.id);
      return;
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
      setProjectsView("library");
      setToast("Project deleted");
      await onRefresh();
    } finally {
      setDeleting(false);
    }
  }

  async function clearProjectWorkspaces() {
    if (!project || !selectedProjectId) return;
    if (!project.workspaces.length) {
      setToast("No workspace folders mapped on this project");
      return;
    }
    const ok = window.confirm(
      `Clear ${project.workspaces.length} workspace path(s) from “${project.name}”? Future Cursor chats in those folders will no longer auto-attribute until remapped.`
    );
    if (!ok) return;
    setClearingWorkspaces(true);
    try {
      await api.clearProjectWorkspaces(selectedProjectId);
      setToast("Workspace listing cleared for this project");
      await onRefresh();
    } catch (e) {
      setToast(e instanceof Error ? e.message : String(e));
    } finally {
      setClearingWorkspaces(false);
    }
  }

  async function syncTasksFromChats() {
    if (!project || !selectedProjectId) return;
    setSyncingTasks(true);
    try {
      const res = await api.syncProjectTasks(selectedProjectId);
      setToast(res.detail);
      await onRefresh();
    } catch (e) {
      setToast(e instanceof Error ? e.message : String(e));
    } finally {
      setSyncingTasks(false);
    }
  }

  const libraryProjects = useMemo(() => {
    const q = librarySearch.trim().toLowerCase();
    if (!q) return projects;
    return projects.filter((p) => {
      const hay = [p.name, p.client, ...(p.workspaces ?? [])]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();
      return hay.includes(q);
    });
  }, [projects, librarySearch]);

  function markFinancesDirty(setter: (v: string) => void) {
    return (v: string) => {
      financesDirtyRef.current = true;
      setter(v);
    };
  }

  async function saveFinances() {
    if (!selectedProjectId) return;
    const budgetMinor = parseMoneyToMinor(budget);
    const revenueMinor = parseMoneyToMinor(revenue);
    const receivedMinor = parseMoneyToMinor(received);
    const otherMinor = parseMoneyToMinor(other);
    if (
      budgetMinor == null ||
      revenueMinor == null ||
      receivedMinor == null ||
      otherMinor == null
    ) {
      setToast("Enter valid amounts for payment / revenue fields");
      return;
    }
    const laborOverride =
      laborRate.trim() === "" || Number(laborRate) <= 0
        ? null
        : Math.round(Number(laborRate) * 100);
    if (laborRate.trim() !== "" && (laborOverride == null || Number.isNaN(laborOverride))) {
      setToast("Enter a valid labor rate or leave it blank");
      return;
    }
    try {
      await api.updateProjectFinances(selectedProjectId, {
        contracted_budget_minor: budgetMinor,
        recognized_revenue_minor: revenueMinor,
        payments_received_minor: receivedMinor,
        other_expenses_minor: otherMinor,
        labor_rate_override_minor: laborOverride,
        currency: projCurrency,
      });
      financesDirtyRef.current = false;
      setToast(`Project finances saved in ${projCurrency}`);
      await onRefresh();
      const fin = await api.getProjectFinancials(selectedProjectId);
      applyFinancialsForm(fin, laborOverride);
    } catch (e) {
      setToast(e instanceof Error ? e.message : String(e));
    }
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

  if (projectsView === "library") {
    return (
      <section className="space-y-4">
        <div className="panel p-5">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="text-lg font-bold">Project library</h2>
              <p className="tiny mt-1">
                Create projects or bulk-sync local folders here. Open a project to manage its tasks,
                finances, workspaces, and attribution.
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              <button type="button" className="btn" onClick={openCreateModal}>
                <Plus className="h-4 w-4" /> New project
              </button>
              <button type="button" className="btn-ghost" onClick={() => setShowSyncLocal(true)}>
                <FolderSync className="h-4 w-4" /> Sync local folders
              </button>
            </div>
          </div>

          <FilterBar>
            <div className="min-w-[160px] flex-1">
              <label className="label">Search projects</label>
              <input
                className="field mt-1"
                value={librarySearch}
                placeholder="Name, client, or workspace path"
                onChange={(e) => setLibrarySearch(e.target.value)}
              />
            </div>
          </FilterBar>

          <div className="mt-2 space-y-2">
            {libraryProjects.length === 0 ? (
              <p className="text-sm text-muted">
                {projects.length === 0
                  ? "No projects yet — create one or sync local folders."
                  : "No projects match your search."}
              </p>
            ) : (
              libraryProjects.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  className="soft flex w-full flex-wrap items-center justify-between gap-3 p-3 text-left transition hover:border-accent/40"
                  onClick={() => openProjectDetail(p.id)}
                >
                  <div className="min-w-0">
                    <div className="font-semibold">{p.name}</div>
                    <div className="tiny mt-0.5">
                      {p.client ? `Org / client: ${p.client}` : "No org / client set"}
                      {p.workspaces.length
                        ? ` · ${p.workspaces.length} workspace${p.workspaces.length === 1 ? "" : "s"}`
                        : " · No workspaces"}
                    </div>
                    {p.workspaces[0] && (
                      <div className="tiny mt-0.5 break-all opacity-80">{p.workspaces[0]}</div>
                    )}
                  </div>
                  <span className="pill shrink-0">Open</span>
                </button>
              ))
            )}
          </div>
        </div>

        {showProject && !editingProject && (
          <Modal
            title="Create project"
            onClose={() => setShowProject(false)}
            onSave={() => void saveProject()}
            saveLabel="Create"
          >
            <Field label="Name" value={pName} onChange={setPName} />
            <Field label="Org / client" value={pClient} onChange={setPClient} />
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
                <button
                  type="button"
                  className="btn-ghost shrink-0"
                  onClick={() => void pickWorkspaceFolder()}
                >
                  <FolderOpen className="h-4 w-4" /> Browse
                </button>
              </div>
              {pWorkspace.trim() && pWorkspaceCheck && (
                <p className={`tiny mt-1 ${pWorkspaceCheck.valid ? "path-ok" : "path-bad"}`}>
                  {pWorkspaceCheck.valid ? "✓ " : "✗ "}
                  {pWorkspaceCheck.detail}
                  {pWorkspaceCheck.normalized ? ` · ${pWorkspaceCheck.normalized}` : ""}
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

        {showSyncLocal && (
          <SyncLocalFoldersModal
            currencies={currencies}
            existingWorkspaceKeys={new Set(
              projects.flatMap((p) => p.workspaces.map((w) => pathKey(w)))
            )}
            onClose={() => setShowSyncLocal(false)}
            onDone={async (createdIds) => {
              setShowSyncLocal(false);
              await onRefresh();
              if (createdIds[0]) openProjectDetail(createdIds[0]);
            }}
          />
        )}
      </section>
    );
  }

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap gap-2">
        <button type="button" className="btn-ghost" onClick={() => setProjectsView("library")}>
          <ArrowLeft className="h-4 w-4" /> All projects
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
          disabled={!project || !project.workspaces.length || clearingWorkspaces}
          onClick={() => void clearProjectWorkspaces()}
        >
          {clearingWorkspaces ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Unlink className="h-4 w-4" />
          )}
          Clear workspaces
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

      {!project ? (
        <div className="panel p-8 text-sm text-muted">
          No project selected.{" "}
          <button type="button" className="font-semibold text-accent" onClick={() => setProjectsView("library")}>
            Open the project library
          </button>
        </div>
      ) : null}

      <div className="grid gap-4 lg:grid-cols-3">
        <div className="panel p-5 lg:col-span-2">
              <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="text-lg font-bold">Tasks & conversations</h2>
              <p className="tiny mt-1">
                Tasks are created from Cursor chats in mapped workspaces. Use{" "}
                <strong>Sync tasks from chats</strong> to create/link a task for every chat already
                seen on this project.
              </p>
            </div>
            <button
              type="button"
              className="btn shrink-0"
              disabled={!project || syncingTasks}
              onClick={() => void syncTasksFromChats()}
            >
              {syncingTasks ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RefreshCw className="h-4 w-4" />
              )}
              {syncingTasks ? "Syncing…" : "Sync tasks from chats"}
            </button>
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
                <div className="tiny mt-1">
                  Org / client: {project.client?.trim() ? project.client : "—"}
                </div>
                <div className="mt-3">
                  <div className="flex items-center justify-between gap-2">
                    <div className="label">Workspace listing</div>
                    {project.workspaces.length > 0 && (
                      <button
                        type="button"
                        className="tiny font-semibold text-accent"
                        disabled={clearingWorkspaces}
                        onClick={() => void clearProjectWorkspaces()}
                      >
                        Clear all
                      </button>
                    )}
                  </div>
                  {project.workspaces.length === 0 ? (
                    <p className="tiny mt-1">None mapped</p>
                  ) : (
                    <ul className="mt-1 space-y-1">
                      {project.workspaces.map((w) => (
                        <li key={w} className="tiny break-all">
                          {w}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </div>
              <div>
                <label className="label">Project currency</label>
                <select
                  className="field mt-1"
                  value={projCurrency}
                  onChange={(e) => {
                    financesDirtyRef.current = true;
                    setProjCurrency(e.target.value);
                  }}
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
                onChange={markFinancesDirty(setBudget)}
                placeholder="0"
              />
              <Field
                label={`Recognized revenue (${ccy})`}
                value={revenue}
                onChange={markFinancesDirty(setRevenue)}
                placeholder="0"
              />
              <Field
                label={`Received payment (${ccy})`}
                value={received}
                onChange={markFinancesDirty(setReceived)}
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
                  onChange={(e) => markFinancesDirty(setLaborRate)(e.target.value)}
                />
                <p className="tiny mt-1">
                  Computed labor for tracked time:{" "}
                  <strong>{money(computedLabor, ccy)}</strong>
                </p>
              </div>
              <Field
                label={`Other expenses (${ccy}, optional)`}
                value={other}
                onChange={markFinancesDirty(setOther)}
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

      {project && (
        <div className="panel p-5">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="font-bold">Attribution policy · {project.name}</h2>
              <p className="tiny mt-1">
                Org / client: {project.client?.trim() ? project.client : "—"} · Only activity for{" "}
                <strong>{project.name}</strong> (its mapped workspaces / linked chats). Other
                projects such as FlowLedger never appear here.
              </p>
              <p className="mt-2 text-sm text-muted">
                Assigning maps a workspace folder onto <strong>{project.name}</strong> and creates a
                General task when needed so future Cursor chats on that folder attribute here.
              </p>
            </div>
          </div>
          <UnassignedReview
            onRefresh={onRefresh}
            projectId={selectedProjectId!}
            projectName={project.name}
          />
        </div>
      )}

      {showProject && editingProject && (
        <Modal
          title="Edit project"
          onClose={() => setShowProject(false)}
          onSave={() => void saveProject()}
          saveLabel="Save changes"
        >
          <Field label="Name" value={pName} onChange={setPName} />
          <Field label="Org / client" value={pClient} onChange={setPClient} />
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
            <p className="tiny mt-1">
              Leave blank and save to clear the primary workspace, or use Clear workspaces above to
              remove all mapped paths.
            </p>
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

function SyncLocalFoldersModal({
  currencies,
  existingWorkspaceKeys,
  onClose,
  onDone,
}: {
  currencies: Array<{ code: string; name: string; symbol: string }>;
  existingWorkspaceKeys: Set<string>;
  onClose: () => void;
  onDone: (createdIds: string[]) => Promise<void>;
}) {
  const setToast = useAppStore((s) => s.setToast);
  const [parentPath, setParentPath] = useState("");
  const [children, setChildren] = useState<ChildDirectory[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [client, setClient] = useState("");
  const [budget, setBudget] = useState("0");
  const [currency, setCurrency] = useState("USD");
  const [scanning, setScanning] = useState(false);
  const [creating, setCreating] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);
  const [progress, setProgress] = useState<{
    current: number;
    total: number;
    name: string;
    done: number;
    failed: number;
  } | null>(null);
  const [rowStatus, setRowStatus] = useState<
    Record<string, "pending" | "working" | "ok" | "fail">
  >({});

  const rows = useMemo(
    () =>
      children.map((c) => ({
        ...c,
        alreadyMapped: existingWorkspaceKeys.has(pathKey(c.path)),
      })),
    [children, existingWorkspaceKeys]
  );

  const selectable = rows.filter((r) => !r.alreadyMapped);
  const selectedCount = selectable.filter((r) => selected.has(r.path)).length;
  const allSelectableChecked =
    selectable.length > 0 && selectable.every((r) => selected.has(r.path));

  function yieldUi() {
    return new Promise<void>((resolve) => {
      window.requestAnimationFrame(() => window.setTimeout(resolve, 0));
    });
  }

  async function pickParentFolder() {
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "Select parent folder with project directories",
      });
      if (typeof picked === "string" && picked) {
        setParentPath(picked);
      }
    } catch {
      setToast("Folder picker unavailable — paste an absolute path");
    }
  }

  async function scanFolders() {
    if (!parentPath.trim()) {
      setToast("Enter or browse to a parent folder");
      return;
    }
    setScanning(true);
    setScanError(null);
    try {
      const list = await api.listChildDirectories(parentPath.trim());
      setChildren(list);
      const next = new Set<string>();
      for (const c of list) {
        if (!existingWorkspaceKeys.has(pathKey(c.path))) next.add(c.path);
      }
      setSelected(next);
      if (list.length === 0) {
        setToast("No subfolders found in that directory");
      }
    } catch (e) {
      setChildren([]);
      setSelected(new Set());
      setScanError(e instanceof Error ? e.message : String(e));
    } finally {
      setScanning(false);
    }
  }

  function toggleAll(checked: boolean) {
    if (!checked) {
      setSelected(new Set());
      return;
    }
    setSelected(new Set(selectable.map((r) => r.path)));
  }

  function toggleOne(path: string, checked: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (checked) next.add(path);
      else next.delete(path);
      return next;
    });
  }

  async function createSelected() {
    const toCreate = selectable.filter((r) => selected.has(r.path));
    if (toCreate.length === 0) {
      setToast("Select at least one folder");
      return;
    }
    setCreating(true);
    const initialStatus: Record<string, "pending" | "working" | "ok" | "fail"> = {};
    for (const folder of toCreate) initialStatus[folder.path] = "pending";
    setRowStatus(initialStatus);
    setProgress({
      current: 0,
      total: toCreate.length,
      name: toCreate[0]?.name ?? "",
      done: 0,
      failed: 0,
    });
    await yieldUi();

    const createdIds: string[] = [];
    let failed = 0;
    let done = 0;
    try {
      const budgetMinor = Math.round(Number(budget || 0) * 100);
      const clientVal = client.trim() || null;
      for (let i = 0; i < toCreate.length; i++) {
        const folder = toCreate[i];
        setProgress({
          current: i + 1,
          total: toCreate.length,
          name: folder.name,
          done,
          failed,
        });
        setRowStatus((prev) => ({ ...prev, [folder.path]: "working" }));
        await yieldUi();
        try {
          const created = await api.createProject({
            name: folder.name,
            client: clientVal,
            description: null,
            git_repository: null,
            billing_model: "fixed_price",
            contracted_budget_minor: budgetMinor,
            currency,
            workspaces: [folder.path],
          });
          createdIds.push(created.id);
          done += 1;
          setRowStatus((prev) => ({ ...prev, [folder.path]: "ok" }));
        } catch {
          failed += 1;
          setRowStatus((prev) => ({ ...prev, [folder.path]: "fail" }));
        }
        setProgress({
          current: i + 1,
          total: toCreate.length,
          name: folder.name,
          done,
          failed,
        });
        await yieldUi();
      }
      if (createdIds.length === 0) {
        setToast(failed ? `Failed to create ${failed} project(s)` : "Nothing created");
        return;
      }
      setToast(
        failed
          ? `Created ${createdIds.length} project(s); ${failed} failed`
          : `Created ${createdIds.length} project(s) from local folders`
      );
      setProgress((p) =>
        p ? { ...p, name: "Refreshing project list…" } : p
      );
      await yieldUi();
      await onDone(createdIds);
    } finally {
      setCreating(false);
      setProgress(null);
    }
  }

  const busyPct =
    progress && progress.total > 0
      ? Math.round(((progress.done + progress.failed) / progress.total) * 100)
      : 0;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#030913c9] p-4 backdrop-blur-sm">
      <div className="panel flex max-h-[90vh] w-full max-w-2xl flex-col p-6 shadow-2xl">
        <h2 className="text-lg font-bold">Sync local folders</h2>
        <p className="tiny mt-1">
          Pick a parent directory, choose which subfolders become projects, then set a shared
          client and budget. Each selected folder is created as its own project with that absolute
          path as the workspace.
        </p>

        {creating && progress && (
          <div className="soft mt-4 space-y-2 p-3" aria-live="polite">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Loader2 className="h-4 w-4 shrink-0 animate-spin text-accent" />
              Creating project {progress.current} of {progress.total}
            </div>
            <p className="tiny break-all">
              {progress.name}
              {progress.done || progress.failed
                ? ` · ${progress.done} ok${progress.failed ? `, ${progress.failed} failed` : ""}`
                : ""}
            </p>
            <div className="h-2 overflow-hidden rounded-full bg-black/30">
              <div
                className="h-full rounded-full bg-accent transition-[width] duration-200"
                style={{
                  width: `${Math.max(busyPct, progress.current > 0 ? 8 : 0)}%`,
                }}
              />
            </div>
          </div>
        )}

        <div
          className={`mt-5 min-h-0 flex-1 space-y-4 overflow-y-auto ${creating ? "pointer-events-none opacity-60" : ""}`}
        >
          <div>
            <label className="label">Parent folder</label>
            <div className="mt-1 flex flex-wrap gap-2">
              <input
                className="field min-w-[12rem] flex-1"
                value={parentPath}
                placeholder="e.g. C:\laragon\www"
                disabled={creating}
                onChange={(e) => setParentPath(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void scanFolders();
                }}
              />
              <button
                type="button"
                className="btn-ghost shrink-0"
                disabled={creating}
                onClick={() => void pickParentFolder()}
              >
                <FolderOpen className="h-4 w-4" /> Browse
              </button>
              <button
                type="button"
                className="btn shrink-0"
                disabled={scanning || creating}
                onClick={() => void scanFolders()}
              >
                {scanning ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" /> Scanning…
                  </>
                ) : (
                  "Scan folders"
                )}
              </button>
            </div>
            {scanError && <p className="tiny path-bad mt-1">{scanError}</p>}
          </div>

          {rows.length > 0 && (
            <div>
              <div className="flex flex-wrap items-center justify-between gap-2">
                <label className="label">Folders to sync ({selectedCount} selected)</label>
                <div className="flex gap-2">
                  <button
                    type="button"
                    className="btn-ghost"
                    disabled={selectable.length === 0 || creating}
                    onClick={() => toggleAll(true)}
                  >
                    Select all
                  </button>
                  <button
                    type="button"
                    className="btn-ghost"
                    disabled={selectedCount === 0 || creating}
                    onClick={() => toggleAll(false)}
                  >
                    Clear
                  </button>
                </div>
              </div>
              <div className="soft mt-2 max-h-56 overflow-y-auto p-2">
                <label className="flex cursor-pointer items-center gap-2 border-b border-white/5 px-2 py-2 text-sm font-semibold">
                  <input
                    type="checkbox"
                    checked={allSelectableChecked}
                    disabled={selectable.length === 0 || creating}
                    onChange={(e) => toggleAll(e.target.checked)}
                  />
                  Select all available
                </label>
                {rows.map((row) => {
                  const st = rowStatus[row.path];
                  return (
                    <label
                      key={row.path}
                      className={`flex items-start gap-2 px-2 py-2 text-sm ${
                        row.alreadyMapped || creating ? "opacity-50" : "cursor-pointer"
                      }`}
                    >
                      <input
                        type="checkbox"
                        className="mt-1"
                        disabled={row.alreadyMapped || creating}
                        checked={row.alreadyMapped ? false : selected.has(row.path)}
                        onChange={(e) => toggleOne(row.path, e.target.checked)}
                      />
                      <span className="min-w-0 flex-1">
                        <span className="flex items-center gap-2 font-semibold">
                          {row.name}
                          {st === "working" && (
                            <Loader2 className="h-3.5 w-3.5 animate-spin text-accent" />
                          )}
                          {st === "ok" && <Check className="h-3.5 w-3.5 path-ok" />}
                          {st === "fail" && <X className="h-3.5 w-3.5 path-bad" />}
                        </span>
                        <span className="tiny mt-0.5 block break-all">{row.path}</span>
                        {row.alreadyMapped && (
                          <span className="tiny mt-0.5 block">Already mapped to a project</span>
                        )}
                        {st === "working" && (
                          <span className="tiny mt-0.5 block">Creating project…</span>
                        )}
                        {st === "fail" && (
                          <span className="tiny path-bad mt-0.5 block">Create failed</span>
                        )}
                      </span>
                    </label>
                  );
                })}
              </div>
            </div>
          )}

          <fieldset disabled={creating} className="space-y-4 border-0 p-0">
            <Field label="Client (applied to all)" value={client} onChange={setClient} />
            <div>
              <label className="label">Currency</label>
              <select
                className="field mt-1"
                value={currency}
                onChange={(e) => setCurrency(e.target.value)}
              >
                {currencies.map((c) => (
                  <option key={c.code} value={c.code}>
                    {c.code} — {c.name}
                  </option>
                ))}
              </select>
            </div>
            <Field
              label={`Total payment / budget (${currency}, applied to all)`}
              value={budget}
              onChange={setBudget}
            />
          </fieldset>
        </div>

        <div className="mt-6 flex justify-end gap-2">
          <button type="button" className="btn-ghost" disabled={creating} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn"
            disabled={creating || selectedCount === 0}
            onClick={() => void createSelected()}
          >
            {creating && progress ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                {progress.current}/{progress.total}…
              </>
            ) : (
              `Create ${selectedCount || ""} project${selectedCount === 1 ? "" : "s"}`.trim()
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

function UnassignedReview({
  onRefresh,
  projectId,
  projectName,
}: {
  onRefresh: () => Promise<void>;
  projectId: string;
  projectName: string;
}) {
  const tasks = useAppStore((s) => s.tasks);
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
  const [clearingAttribution, setClearingAttribution] = useState(false);

  async function load(nextPage = page) {
    if (!projectId) {
      setItems([]);
      setTotal(0);
      setReasons([]);
      return;
    }
    setLoading(true);
    try {
      const res = await api.listUnassignedPage({
        projectId,
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
  }, [reason, conversation, search, pageSize, projectId]);

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  async function assignToProject(itemId: string) {
    if (!projectId) return;
    setAssigningId(itemId);
    try {
      // Prefer an existing non-General task when present; otherwise backend creates General.
      const preferred =
        tasks.find((t) => t.title !== "General")?.id ?? tasks[0]?.id ?? null;
      await api.assignUnassigned(itemId, projectId, preferred);
      setToast(`Assigned to ${projectName} (workspace mapped)`);
      await onRefresh();
      await load(page);
    } catch (e) {
      setToast(e instanceof Error ? e.message : "Assign failed");
    } finally {
      setAssigningId(null);
    }
  }

  async function clearProjectAttribution() {
    if (!projectId) return;
    const ok = window.confirm(
      `Clear the attribution queue for “${projectName}”? Pending review rows for this project will be dismissed (they are not deleted from activity history).`
    );
    if (!ok) return;
    setClearingAttribution(true);
    try {
      const n = await api.dismissUnassignedForProject(projectId);
      setToast(
        n === 0
          ? `No pending attribution rows for ${projectName}`
          : `Cleared ${n} attribution row(s) for ${projectName}`
      );
      await onRefresh();
      await load(1);
    } catch (e) {
      setToast(e instanceof Error ? e.message : String(e));
    } finally {
      setClearingAttribution(false);
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
        <button
          type="button"
          className="btn-ghost"
          disabled={clearingAttribution || total === 0}
          onClick={() => void clearProjectAttribution()}
        >
          {clearingAttribution ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" /> Clearing…
            </>
          ) : (
            "Clear attribution queue"
          )}
        </button>
      </FilterBar>

      <p className="tiny">
        {loading ? "Loading…" : `${total} pending for ${projectName}`} · page {page} of {totalPages}
      </p>

      {items.length === 0 ? (
        <p className="tiny">No pending unassigned activity for {projectName}.</p>
      ) : (
        items.map((item) => (
          <div key={item.id} className="soft flex flex-wrap items-center justify-between gap-3 p-3">
            <div>
              <div className="text-sm font-semibold">{item.reason}</div>
              <div className="tiny">
                {item.workspace_path || "—"} · {item.conversation_id || "no conversation"}
              </div>
              <p className="tiny mt-1">
                Assign maps this workspace to {projectName} only and links the conversation.
              </p>
            </div>
            <button
              type="button"
              className="btn"
              disabled={assigningId === item.id}
              onClick={() => void assignToProject(item.id)}
            >
              {assigningId === item.id ? "Assigning…" : `Assign to ${projectName}`}
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
