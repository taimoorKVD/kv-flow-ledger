import { create } from "zustand";
import type { DashboardSummary, Project, Task } from "@/lib/api";

export type PageId =
  | "overview"
  | "projects"
  | "activity"
  | "usage"
  | "finance"
  | "settings";

/** Library = create / sync / browse all; Detail = selected project only */
export type ProjectsView = "library" | "detail";

type AppStore = {
  page: PageId;
  setPage: (p: PageId) => void;
  projectsView: ProjectsView;
  setProjectsView: (v: ProjectsView) => void;
  projectsCreateRequest: number;
  requestProjectsCreate: () => void;
  projects: Project[];
  setProjects: (p: Project[]) => void;
  selectedProjectId: string | null;
  setSelectedProjectId: (id: string | null) => void;
  tasks: Task[];
  setTasks: (t: Task[]) => void;
  dashboard: DashboardSummary | null;
  setDashboard: (d: DashboardSummary | null) => void;
  theme: "dark" | "light";
  setTheme: (t: "dark" | "light") => void;
  error: string | null;
  setError: (e: string | null) => void;
  loading: boolean;
  setLoading: (v: boolean) => void;
  toast: string | null;
  setToast: (t: string | null) => void;
};

export const useAppStore = create<AppStore>((set) => ({
  page: "overview",
  setPage: (page) => set({ page }),
  projectsView: "library",
  setProjectsView: (projectsView) => set({ projectsView }),
  projectsCreateRequest: 0,
  requestProjectsCreate: () =>
    set((s) => ({
      page: "projects",
      projectsView: "library",
      projectsCreateRequest: s.projectsCreateRequest + 1,
    })),
  projects: [],
  setProjects: (projects) => set({ projects }),
  selectedProjectId: null,
  setSelectedProjectId: (selectedProjectId) => set({ selectedProjectId }),
  tasks: [],
  setTasks: (tasks) => set({ tasks }),
  dashboard: null,
  setDashboard: (dashboard) => set({ dashboard }),
  theme: "dark",
  setTheme: (theme) => set({ theme }),
  error: null,
  setError: (error) => set({ error }),
  loading: true,
  setLoading: (loading) => set({ loading }),
  toast: null,
  setToast: (toast) => set({ toast }),
}));

export const pageMeta: Record<PageId, [string, string]> = {
  overview: [
    "Project intelligence",
    "Continuous task time, verifiable AI spending and project profitability.",
  ],
  projects: [
    "Projects & task attribution",
    "Create projects in the library, then manage tasks and attribution per project.",
  ],
  activity: [
    "Activity timeline",
    "Auditable work and agent activity, with idle gaps excluded.",
  ],
  usage: [
    "AI usage ledger",
    "Source-aware tokens and costs, without inventing billing data.",
  ],
  finance: [
    "Financial performance",
    "Separate project budget, earned revenue and actual cash received.",
  ],
  settings: [
    "Tracking policies",
    "Configure autonomous time recognition and labor valuation.",
  ],
};
