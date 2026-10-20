import { useCallback, useEffect } from "react";
import { Plus, ShieldCheck } from "lucide-react";
import { api } from "@/lib/api";
import { BrandMark, BrandWordmark } from "@/components/BrandMark";
import { pageMeta, useAppStore, type PageId } from "@/stores/appStore";
import OverviewPage from "@/pages/OverviewPage";
import ProjectsPage from "@/pages/ProjectsPage";
import ActivityPage from "@/pages/ActivityPage";
import UsagePage from "@/pages/UsagePage";
import FinancePage from "@/pages/FinancePage";
import SettingsPage from "@/pages/SettingsPage";

const nav: { id: PageId; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "projects", label: "Projects & Tasks" },
  { id: "activity", label: "Activity Timeline" },
  { id: "usage", label: "AI Usage" },
  { id: "finance", label: "Financials" },
  { id: "settings", label: "Settings" },
];

export default function App() {
  const page = useAppStore((s) => s.page);
  const setPage = useAppStore((s) => s.setPage);
  const projects = useAppStore((s) => s.projects);
  const setProjects = useAppStore((s) => s.setProjects);
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const setSelectedProjectId = useAppStore((s) => s.setSelectedProjectId);
  const setTasks = useAppStore((s) => s.setTasks);
  const dashboard = useAppStore((s) => s.dashboard);
  const setDashboard = useAppStore((s) => s.setDashboard);
  const theme = useAppStore((s) => s.theme);
  const error = useAppStore((s) => s.error);
  const setError = useAppStore((s) => s.setError);
  const loading = useAppStore((s) => s.loading);
  const setLoading = useAppStore((s) => s.setLoading);
  const toast = useAppStore((s) => s.toast);
  const setToast = useAppStore((s) => s.setToast);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const ps = await api.listProjects();
      setProjects(ps);
      const pid = selectedProjectId && ps.some((p) => p.id === selectedProjectId)
        ? selectedProjectId
        : ps[0]?.id ?? null;
      if (pid !== selectedProjectId) setSelectedProjectId(pid);
      const [tasks, dash] = await Promise.all([
        api.listTasks(pid ?? undefined),
        api.getDashboard(pid ?? undefined),
      ]);
      setTasks(tasks);
      setDashboard(dash);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [
    selectedProjectId,
    setDashboard,
    setError,
    setLoading,
    setProjects,
    setSelectedProjectId,
    setTasks,
  ]);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    document.documentElement.classList.toggle("light", theme === "light");
  }, [theme]);

  useEffect(() => {
    void refresh();
    const t = window.setInterval(() => void refresh(), 5000);
    return () => window.clearInterval(t);
  }, [refresh]);

  useEffect(() => {
    if (!toast) return;
    const t = window.setTimeout(() => setToast(null), 2800);
    return () => window.clearTimeout(t);
  }, [toast, setToast]);

  const [title, sub] = pageMeta[page];
  const tracking = dashboard?.tracking;
  const connected = Boolean(tracking?.ingest_listening);

  return (
    <div className="min-h-screen">
      <header className="app-header sticky top-0 z-30 border-b backdrop-blur-xl">
        <div className="shell px-5 lg:px-9">
          <div className="flex h-[67px] items-center justify-between gap-4">
            <div className="flex min-w-0 items-center gap-3">
              <div className="brand-mark grid h-9 w-9 place-items-center rounded-xl">
                <BrandMark className="h-[18px] w-[18px]" />
              </div>
              <BrandWordmark />
            </div>
            <div className="flex items-center gap-2">
              <span className="soft hidden items-center gap-2 px-3 py-2 text-xs md:flex">
                <span className={`dot ${connected ? "" : "off"}`} />
                <span>
                  {connected
                    ? tracking?.hooks_installed
                      ? "Tracking live"
                      : "Ingest ready · hooks not installed"
                    : "Disconnected"}
                </span>
              </span>
            </div>
          </div>
          <div className="flex items-center justify-between gap-3 pb-2">
            <nav className="flex min-w-0 gap-1 overflow-x-auto">
              {nav.map((n) => (
                <button
                  key={n.id}
                  type="button"
                  className={`navtab ${page === n.id ? "active" : ""}`}
                  onClick={() => setPage(n.id)}
                >
                  {n.label}
                </button>
              ))}
            </nav>
            <div className="hidden items-center gap-2 text-xs text-muted xl:flex">
              <ShieldCheck className="h-4 w-4 text-accent" /> Local-first · No cloud upload
            </div>
          </div>
        </div>
      </header>

      <main className="shell space-y-6 px-5 py-8 lg:px-9">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div>
            <div className="mb-2 text-xs font-semibold tracking-[.14em] text-accent">
              WORKSPACE / ENGINEERING
            </div>
            <h1 className="text-[27px] font-bold tracking-tight sm:text-[32px]">{title}</h1>
            <p className="mt-1 text-sm text-muted">{sub}</p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <div className="soft flex items-center gap-2 px-3 py-2">
              <select
                className="max-w-[190px] bg-transparent text-sm font-semibold outline-none"
                value={selectedProjectId ?? ""}
                onChange={(e) => setSelectedProjectId(e.target.value || null)}
              >
                {projects.length === 0 && <option value="">No projects</option>}
                {projects.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </div>
            <button
              type="button"
              className="btn"
              onClick={() => setPage("projects")}
            >
              <Plus className="h-4 w-4" /> New project
            </button>
          </div>
        </div>

        {error && (
          <div className="panel border-red-900/50 p-4 text-sm text-red-300">
            {error}
          </div>
        )}
        {loading && !dashboard ? (
          <div className="panel p-8 text-sm text-muted">Loading local data…</div>
        ) : (
          <>
            {page === "overview" && <OverviewPage onRefresh={refresh} />}
            {page === "projects" && <ProjectsPage onRefresh={refresh} />}
            {page === "activity" && <ActivityPage />}
            {page === "usage" && <UsagePage onRefresh={refresh} />}
            {page === "finance" && <FinancePage />}
            {page === "settings" && <SettingsPage onRefresh={refresh} />}
          </>
        )}
      </main>

      {toast && (
        <div className="toast fixed bottom-6 right-6 z-40 rounded-xl border px-4 py-3 text-sm text-white shadow-xl">
          {toast}
        </div>
      )}
    </div>
  );
}
