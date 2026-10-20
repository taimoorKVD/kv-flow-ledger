import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import { money } from "@/lib/format";
import { FilterBar, PaginationBar, useClientPage } from "@/components/ListControls";
import { useAppStore } from "@/stores/appStore";

const RATE_PAGE_SIZE = 8;

export default function SettingsPage({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const setToast = useAppStore((s) => s.setToast);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const [idle, setIdle] = useState("5");
  const [salary, setSalary] = useState("0");
  const [hours, setHours] = useState("160");
  const [salaryCcy, setSalaryCcy] = useState("USD");
  const [review, setReview] = useState(false);
  const [hooks, setHooks] = useState<Record<string, unknown> | null>(null);
  const [appInfo, setAppInfo] = useState<Record<string, unknown> | null>(null);
  const [currencies, setCurrencies] = useState<
    Array<{ code: string; name: string; symbol: string }>
  >([]);
  const [rates, setRates] = useState<
    Array<{
      id: string;
      base_currency: string;
      quote_currency: string;
      rate_numeric: string;
      effective_on: string;
    }>
  >([]);
  const [rateBase, setRateBase] = useState("USD");
  const [rateQuote, setRateQuote] = useState("PKR");
  const [rateValue, setRateValue] = useState("278.50");
  const [rateSearch, setRateSearch] = useState("");
  const [ratePage, setRatePage] = useState(1);
  const [cursorApiKey, setCursorApiKey] = useState("");
  const [cursorSyncDetail, setCursorSyncDetail] = useState("");
  const [cursorSyncAt, setCursorSyncAt] = useState("");
  const [cursorBusy, setCursorBusy] = useState(false);

  useEffect(() => {
    void (async () => {
      const s = await api.getSettings();
      setIdle(s.values.idle_threshold_minutes ?? "5");
      setSalary(String(Number(s.values.monthly_salary_minor ?? "0") / 100));
      setHours(s.values.monthly_hours ?? "160");
      setSalaryCcy(s.values.default_currency ?? "USD");
      setReview((s.values.require_attribution_review ?? "false") === "true");
      if (s.values.theme === "light" || s.values.theme === "dark") setTheme(s.values.theme);
      setCursorApiKey(s.values.cursor_api_key ?? "");
      setCursorSyncAt(s.values.cursor_usage_last_sync_at ?? "");
      setCursorSyncDetail(s.values.cursor_usage_last_sync_detail ?? "");
      setHooks(await api.hooksStatus());
      setAppInfo(await api.getAppInfo());
      setCurrencies(await api.listCurrencies());
      setRates(await api.listExchangeRates());
    })();
  }, [setTheme]);

  const perMinute =
    Number(hours) > 0 ? Math.round((Number(salary) * 100) / Number(hours) / 60) : 0;

  const filteredRates = rates.filter((r) => {
    if (!rateSearch.trim()) return true;
    const q = rateSearch.trim().toLowerCase();
    return `${r.base_currency} ${r.quote_currency} ${r.rate_numeric}`
      .toLowerCase()
      .includes(q);
  });
  const { slice: pagedRates, total: rateTotal, totalPages: ratePages, page: safeRatePage } =
    useClientPage(filteredRates, ratePage, RATE_PAGE_SIZE);

  useEffect(() => {
    setRatePage(1);
  }, [rateSearch]);

  const hooksInstalled = Boolean(hooks?.installed);

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
      default_currency: salaryCcy,
    });
    setToast(`Labor valuation saved (${salaryCcy})`);
    await onRefresh();
  }

  async function saveRate() {
    await api.upsertExchangeRate({
      baseCurrency: rateBase,
      quoteCurrency: rateQuote,
      rateNumeric: rateValue,
    });
    setRates(await api.listExchangeRates());
    setToast(`Rate ${rateBase}→${rateQuote} = ${rateValue} saved`);
    await onRefresh();
  }

  async function saveCursorApiKey() {
    await api.saveSettings({ cursor_api_key: cursorApiKey.trim() });
    setToast(cursorApiKey.trim() ? "Cursor API key saved" : "Cursor API key cleared");
    await onRefresh();
  }

  async function testCursorKey() {
    setCursorBusy(true);
    try {
      await api.saveSettings({ cursor_api_key: cursorApiKey.trim() });
      const msg = await api.testCursorApiKey();
      setToast(msg);
    } catch (e) {
      setToast(e instanceof Error ? e.message : String(e));
    } finally {
      setCursorBusy(false);
    }
  }

  async function syncCursorUsage() {
    setCursorBusy(true);
    try {
      await api.saveSettings({ cursor_api_key: cursorApiKey.trim() });
      const res = await api.syncCursorUsage(30, null);
      setCursorSyncDetail(res.detail);
      setCursorSyncAt(new Date().toISOString());
      setToast(res.detail);
      await onRefresh();
    } catch (e) {
      setToast(e instanceof Error ? e.message : String(e));
    } finally {
      setCursorBusy(false);
    }
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
                className="mt-1 accent-[var(--accent)]"
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
          <h2 className="text-lg font-bold">Labor valuation (global default)</h2>
          <p className="tiny mt-1">
            Used when a project has no labor rate override. Per-project rates live on Projects.
          </p>
          <div className="mt-5 space-y-4">
            <div>
              <label className="label">Salary currency</label>
              <select
                className="field mt-1"
                value={salaryCcy}
                onChange={(e) => setSalaryCcy(e.target.value)}
              >
                {currencies.map((c) => (
                  <option key={c.code} value={c.code}>
                    {c.code} — {c.name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="label">Monthly salary / cost basis ({salaryCcy})</label>
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
              <div className="mt-1 text-2xl font-bold">{money(perMinute, salaryCcy)}</div>
            </div>
            <button type="button" className="btn" onClick={() => void saveSalary()}>
              Save valuation
            </button>
            <p className="tiny">
              When a project uses another currency (e.g. PKR), labor cost is converted using
              exchange rates below unless that project sets its own hourly override.
            </p>
          </div>
        </div>
      </div>

      <div className="panel p-5">
        <h2 className="text-lg font-bold">Exchange rates</h2>
        <p className="tiny mt-1">
          Rate = quote units per 1 base unit (example: USD→PKR 278.50 means 1 USD = 278.50 PKR).
        </p>
        <div className="mt-4 grid gap-3 sm:grid-cols-4">
          <div>
            <label className="label">Base</label>
            <select className="field mt-1" value={rateBase} onChange={(e) => setRateBase(e.target.value)}>
              {currencies.map((c) => (
                <option key={c.code} value={c.code}>
                  {c.code}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="label">Quote</label>
            <select className="field mt-1" value={rateQuote} onChange={(e) => setRateQuote(e.target.value)}>
              {currencies.map((c) => (
                <option key={c.code} value={c.code}>
                  {c.code}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="label">Rate</label>
            <input className="field mt-1" value={rateValue} onChange={(e) => setRateValue(e.target.value)} />
          </div>
          <div className="flex items-end">
            <button type="button" className="btn w-full" onClick={() => void saveRate()}>
              Save rate
            </button>
          </div>
        </div>
        <FilterBar>
          <div className="min-w-[160px] flex-1">
            <label className="label">Filter rates</label>
            <input
              className="field mt-1"
              value={rateSearch}
              placeholder="USD, PKR…"
              onChange={(e) => setRateSearch(e.target.value)}
            />
          </div>
        </FilterBar>
        <div className="overflow-x-auto">
          <table className="table">
            <thead>
              <tr>
                <th>Pair</th>
                <th>Rate</th>
                <th>Effective</th>
              </tr>
            </thead>
            <tbody>
              {pagedRates.length === 0 && (
                <tr>
                  <td colSpan={3}>No rates yet</td>
                </tr>
              )}
              {pagedRates.map((r) => (
                <tr key={r.id}>
                  <td>
                    {r.base_currency} → {r.quote_currency}
                  </td>
                  <td>{r.rate_numeric}</td>
                  <td>{r.effective_on}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <PaginationBar
          page={safeRatePage}
          totalPages={ratePages}
          total={rateTotal}
          pageSize={RATE_PAGE_SIZE}
          onPage={setRatePage}
        />
      </div>

      <div className={`panel p-5 ${cursorApiKey.trim() ? "status-ok-panel" : "status-warn-panel"}`}>
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 className="text-lg font-bold">Cursor usage &amp; cost API</h2>
            <p className="mt-2 text-sm text-muted">
              Primary source for AI allocation. Uses the Cursor Admin API (
              <code>filtered-usage-events</code>) with your dashboard API key — billed{" "}
              <code>chargedCents</code> and tokens, joined to projects via conversation id from
              hooks. Teams / Enterprise admin keys from{" "}
              <code>cursor.com/dashboard → API Keys</code>.
            </p>
          </div>
          <span className={`status-badge ${cursorApiKey.trim() ? "ok" : "warn"}`}>
            {cursorApiKey.trim() ? "Key saved" : "Key required"}
          </span>
        </div>
        <div className="mt-4 space-y-3">
          <div>
            <label className="label">Cursor Admin API key</label>
            <input
              className="field mt-1 font-mono text-xs"
              type="password"
              autoComplete="off"
              value={cursorApiKey}
              placeholder="crsr_…"
              onChange={(e) => setCursorApiKey(e.target.value)}
            />
          </div>
          {(cursorSyncAt || cursorSyncDetail) && (
            <div className="soft p-3 text-sm">
              <div className="tiny">Last sync: {cursorSyncAt || "—"}</div>
              <div className="mt-1">{cursorSyncDetail || "—"}</div>
            </div>
          )}
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className="btn"
              disabled={cursorBusy || !cursorApiKey.trim()}
              onClick={() => void saveCursorApiKey()}
            >
              Save API key
            </button>
            <button
              type="button"
              className="btn-ghost"
              disabled={cursorBusy || !cursorApiKey.trim()}
              onClick={() => void testCursorKey()}
            >
              Test connection
            </button>
            <button
              type="button"
              className="btn-ghost"
              disabled={cursorBusy || !cursorApiKey.trim()}
              onClick={() => void syncCursorUsage()}
            >
              {cursorBusy ? "Syncing…" : "Sync last 30 days"}
            </button>
          </div>
        </div>
      </div>

      <div className={`panel p-5 ${hooksInstalled ? "status-ok-panel" : "status-warn-panel"}`}>
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 className="text-lg font-bold">Cursor hook installation</h2>
            <p className="mt-2 text-sm text-muted">
              Explicit, reversible install into <code>~/.cursor/hooks.json</code> with backup. Does not
              modify project repositories unless you add project hooks yourself.
            </p>
          </div>
          <span className={`status-badge ${hooksInstalled ? "ok" : "warn"}`}>
            {hooksInstalled ? "Installed" : "Not installed"}
          </span>
        </div>
        <div className={`mt-4 soft p-4 text-sm ${hooksInstalled ? "status-ok-soft" : ""}`}>
          <div className="flex items-center gap-2 font-semibold">
            <span className={`dot ${hooksInstalled ? "" : "off"}`} />
            Status: {hooksInstalled ? "Hooks verified in hooks.json" : "Hooks not present"}
          </div>
          <div className="tiny mt-2">{String(hooks?.hooks_path ?? "")}</div>
          <div className="tiny mt-1">{String(hooks?.detail ?? "")}</div>
          <div className="tiny mt-1">
            Data file: {String(appInfo?.db_path ?? "—")} · Ingest listening:{" "}
            {String(appInfo?.ingest_listening ?? false)}
          </div>
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          <button type="button" className="btn" onClick={() => void installHooks()}>
            {hooksInstalled ? "Reinstall hooks" : "Install hooks"}
          </button>
          <button
            type="button"
            className="btn-ghost"
            disabled={!hooksInstalled}
            onClick={() => void uninstallHooks()}
          >
            Uninstall hooks
          </button>
        </div>
      </div>
    </section>
  );
}
