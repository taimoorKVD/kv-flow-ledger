import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import { money } from "@/lib/format";
import { useAppStore } from "@/stores/appStore";

export default function SettingsPage({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const setToast = useAppStore((s) => s.setToast);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const [idle, setIdle] = useState("5");
  const [salary, setSalary] = useState("0");
  const [hours, setHours] = useState("160");
  const [review, setReview] = useState(true);
  const [hooks, setHooks] = useState<Record<string, unknown> | null>(null);
  const [appInfo, setAppInfo] = useState<Record<string, unknown> | null>(null);

  useEffect(() => {
    void (async () => {
      const s = await api.getSettings();
      setIdle(s.values.idle_threshold_minutes ?? "5");
      setSalary(String(Number(s.values.monthly_salary_minor ?? "0") / 100));
      setHours(s.values.monthly_hours ?? "160");
      setReview((s.values.require_attribution_review ?? "true") === "true");
      if (s.values.theme === "light" || s.values.theme === "dark") setTheme(s.values.theme);
      setHooks(await api.hooksStatus());
      setAppInfo(await api.getAppInfo());
    })();
  }, [setTheme]);

  const perMinute =
    Number(hours) > 0 ? Math.round((Number(salary) * 100) / Number(hours) / 60) : 0;

  async function saveRules() {
    await api.saveSettings({
      idle_threshold_minutes: idle,
      require_attribution_review: review ? "true" : "false",
      theme,
    });
    setToast("Tracking rules saved");
    await onRefresh();
  }

  async function saveSalary() {
    await api.saveSettings({
      monthly_salary_minor: String(Math.round(Number(salary) * 100)),
      monthly_hours: hours,
    });
    setToast("Labor valuation saved");
    await onRefresh();
  }

  async function installHooks() {
    const st = await api.installHooks();
    setHooks(st);
    setToast("Cursor hooks installed (user ~/.cursor/hooks.json)");
    await onRefresh();
  }

  async function uninstallHooks() {
    const st = await api.uninstallHooks();
    setHooks(st);
    setToast("Cursor hooks uninstalled");
    await onRefresh();
  }

  return (
    <section className="space-y-4">
      <div className="grid gap-4 lg:grid-cols-2">
        <div className="panel p-5">
          <h2 className="text-lg font-bold">Tracking & idle rules</h2>
          <p className="tiny mt-1">Applied to the local tracker when connected.</p>
          <div className="mt-5 space-y-5">
            <div>
              <label className="label">Idle threshold (minutes)</label>
              <select className="field mt-1" value={idle} onChange={(e) => setIdle(e.target.value)}>
                {[2, 5, 10, 15].map((n) => (
                  <option key={n} value={n}>
                    {n} minutes
                  </option>
                ))}
              </select>
            </div>
            <div className="soft p-4 text-sm leading-6">
              Task time counts intervals where <strong>editor work OR an agent run</strong> is
              active. An idle user does not pause a running agent. Overlapping intervals count once.
            </div>
            <label className="flex items-start gap-3 text-sm">
              <input
                type="checkbox"
                className="mt-1 accent-blue-400"
                checked={review}
                onChange={(e) => setReview(e.target.checked)}
              />
              <span>Require review for ambiguous task attribution</span>
            </label>
            <div>
              <label className="label">Theme</label>
              <select
                className="field mt-1"
                value={theme}
                onChange={(e) => setTheme(e.target.value as "dark" | "light")}
              >
                <option value="dark">Dark</option>
                <option value="light">Light</option>
              </select>
            </div>
            <button type="button" className="btn" onClick={() => void saveRules()}>
              Save rules
            </button>
          </div>
        </div>

        <div className="panel p-5">
          <h2 className="text-lg font-bold">Labor valuation</h2>
          <p className="tiny mt-1">Task-time valuation; not a claim about keyboard time.</p>
          <div className="mt-5 space-y-4">
            <div>
              <label className="label">Monthly salary / cost basis (USD)</label>
              <input
                className="field mt-1"
                type="number"
                min={0}
                step="0.01"
                value={salary}
                onChange={(e) => setSalary(e.target.value)}
              />
            </div>
            <div>
              <label className="label">Working hours per month</label>
              <input
                className="field mt-1"
                type="number"
                min={1}
                value={hours}
                onChange={(e) => setHours(e.target.value)}
              />
            </div>
            <div className="soft p-4">
              <div className="tiny">Labor valuation per task-work minute</div>
              <div className="mt-1 text-2xl font-bold">{money(perMinute)}</div>
            </div>
            <button type="button" className="btn" onClick={() => void saveSalary()}>
              Save valuation
            </button>
            <p className="tiny">
              Salary-based labor valuation is an imputed economic cost, not an automatic cash
              outflow.
            </p>
          </div>
        </div>
      </div>

      <div className="panel p-5">
        <h2 className="text-lg font-bold">Cursor hook installation</h2>
        <p className="mt-2 text-sm text-muted">
          Explicit, reversible install into <code>~/.cursor/hooks.json</code> with backup. Does not
          modify project repositories unless you add project hooks yourself.
        </p>
        <div className="mt-4 soft p-4 text-sm">
          <div>Status: {hooks?.installed ? "Installed" : "Not installed"}</div>
          <div className="tiny mt-1">{String(hooks?.hooks_path ?? "")}</div>
          <div className="tiny mt-1">{String(hooks?.detail ?? "")}</div>
          <div className="tiny mt-1">
            Data file: {String(appInfo?.db_path ?? "—")} · Ingest listening:{" "}
            {String(appInfo?.ingest_listening ?? false)}
          </div>
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <button type="button" className="btn" onClick={() => void installHooks()}>
            Install hooks
          </button>
          <button type="button" className="btn-ghost" onClick={() => void uninstallHooks()}>
            Uninstall hooks
          </button>
        </div>
      </div>
    </section>
  );
}
