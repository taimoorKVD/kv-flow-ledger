import { useEffect, useState, type ReactNode } from "react";
import { Plus } from "lucide-react";
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
  const [showTask, setShowTask] = useState(false);
  const [pName, setPName] = useState("");
  const [pClient, setPClient] = useState("");
  const [pWorkspace, setPWorkspace] = useState("");
  const [pBudget, setPBudget] = useState("0");
  const [tTitle, setTTitle] = useState("");
  const [tConv, setTConv] = useState("");
  const [budget, setBudget] = useState("");
  const [revenue, setRevenue] = useState("");
  const [received, setReceived] = useState("");
  const [other, setOther] = useState("");

  const project = projects.find((p) => p.id === selectedProjectId);

  async function createProject() {
    if (!pName.trim()) return setToast("Enter project name");
    const created = await api.createProject({
      name: pName.trim(),
      client: pClient.trim() || null,
      description: null,
      git_repository: null,
      billing_model: "fixed_price",
      contracted_budget_minor: Math.round(Number(pBudget || 0) * 100),
      currency: "USD",
      workspaces: pWorkspace.trim() ? [pWorkspace.trim()] : [],
    });
    setSelectedProjectId(created.id);
    setShowProject(false);
    setPName("");
    setToast("Project created");
    await onRefresh();
  }

  async function createTask() {
    if (!selectedProjectId) return setToast("Create a project first");
    if (!tTitle.trim()) return setToast("Enter task title");
    await api.createTask({
      project_id: selectedProjectId,
      title: tTitle.trim(),
      description: null,
      conversation_id: tConv.trim() || null,
    });
    setShowTask(false);
    setTTitle("");
    setTConv("");
    setToast("Task created");
    await onRefresh();
  }

  async function saveFinances() {
    if (!selectedProjectId) return;
    await api.updateProjectFinances(selectedProjectId, {
      contracted_budget_minor: Math.round(Number(budget || 0) * 100),
      recognized_revenue_minor: Math.round(Number(revenue || 0) * 100),
      payments_received_minor: Math.round(Number(received || 0) * 100),
      other_expenses_minor: Math.round(Number(other || 0) * 100),
    });
    setToast("Project finances saved");
    await onRefresh();
  }

  return (
    <section className="space-y-4">
      <div className="flex flex-wrap gap-2">
        <button type="button" className="btn" onClick={() => setShowProject(true)}>
          <Plus className="h-4 w-4" /> New project
        </button>
        <button type="button" className="btn-ghost" onClick={() => setShowTask(true)}>
          <Plus className="h-4 w-4" /> Add task
        </button>
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <div className="panel p-5 lg:col-span-2">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-lg font-bold">Tasks & conversations</h2>
              <p className="tiny mt-1">Task associations persist across Cursor sessions.</p>
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
                    <td colSpan={4}>No tasks</td>
                  </tr>
                )}
                {tasks.map((t) => (
                  <tr key={t.id}>
                    <td>
                      <div className="font-semibold">{t.title}</div>
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
                <div className="tiny mt-1">
                  Workspaces: {project.workspaces.length ? project.workspaces.join(", ") : "None mapped"}
                </div>
              </div>
              <Field label="Contracted budget (USD)" value={budget} onChange={setBudget} placeholder={String(project.contracted_budget_minor / 100)} />
              <Field label="Recognized revenue (USD)" value={revenue} onChange={setRevenue} />
              <Field label="Payments received (USD)" value={received} onChange={setReceived} />
              <Field label="Other expenses (USD)" value={other} onChange={setOther} />
              <button type="button" className="btn w-full" onClick={() => void saveFinances()}>
                Save project finances
              </button>
              <p className="tiny">
                Profit uses recognized revenue. Cash collected and contract budget are reported separately.
              </p>
            </div>
          )}
        </div>
      </div>

      <div className="panel p-5">
        <h2 className="font-bold">Project attribution policy</h2>
        <p className="mt-2 text-sm text-muted">
          Workspace → project; linked conversation → task. Ambiguous sessions remain unassigned until reviewed.
        </p>
        <UnassignedReview onRefresh={onRefresh} />
      </div>

      {showProject && (
        <Modal title="Create project" onClose={() => setShowProject(false)} onSave={() => void createProject()}>
          <Field label="Name" value={pName} onChange={setPName} />
          <Field label="Client" value={pClient} onChange={setPClient} />
          <Field label="Workspace directory (absolute path)" value={pWorkspace} onChange={setPWorkspace} />
          <Field label="Contracted budget (USD)" value={pBudget} onChange={setPBudget} />
        </Modal>
      )}
      {showTask && (
        <Modal title="Create task" onClose={() => setShowTask(false)} onSave={() => void createTask()}>
          <Field label="Task name" value={tTitle} onChange={setTTitle} />
          <Field label="Cursor conversation id (optional)" value={tConv} onChange={setTConv} />
        </Modal>
      )}
    </section>
  );
}

function UnassignedReview({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const [items, setItems] = useState<Array<Record<string, string>>>([]);
  const tasks = useAppStore((s) => s.tasks);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const setToast = useAppStore((s) => s.setToast);

  useEffect(() => {
    void api.listUnassigned().then(setItems).catch(() => setItems([]));
  }, []);

  return (
    <div className="mt-4 space-y-3">
      <button
        type="button"
        className="btn-ghost"
        onClick={() => void api.listUnassigned().then(setItems)}
      >
        Refresh unassigned
      </button>
      {items.length === 0 ? (
        <p className="tiny">No pending unassigned activity.</p>
      ) : (
        items.map((item) => (
          <div key={item.id} className="soft flex flex-wrap items-center justify-between gap-3 p-3">
            <div>
              <div className="text-sm font-semibold">{item.reason}</div>
              <div className="tiny">
                {item.workspace_path || "—"} · {item.conversation_id || "no conversation"}
              </div>
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
                setItems(await api.listUnassigned());
              }}
            >
              Assign to current project task
            </button>
          </div>
        ))
      )}
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
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  onSave: () => void;
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
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
