import { useEffect, useMemo, useState } from "react";
import { api, type ActivityEvent } from "@/lib/api";
import { dur } from "@/lib/format";
import { FilterBar, PaginationBar, useClientPage } from "@/components/ListControls";
import { useAppStore } from "@/stores/appStore";

const PAGE_SIZE = 15;

export default function ActivityPage() {
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const dashboard = useAppStore((s) => s.dashboard);
  const tasks = useAppStore((s) => s.tasks);
  const [events, setEvents] = useState<ActivityEvent[]>([]);
  const [search, setSearch] = useState("");
  const [eventType, setEventType] = useState("all");
  const [page, setPage] = useState(1);

  useEffect(() => {
    void api
      .listActivity(selectedProjectId ?? undefined)
      .then(setEvents)
      .catch(() => setEvents([]));
  }, [selectedProjectId]);

  useEffect(() => {
    setPage(1);
  }, [search, eventType, selectedProjectId]);

  const types = useMemo(
    () => Array.from(new Set(events.map((e) => e.event_type))).sort(),
    [events]
  );

  const filtered = useMemo(() => {
    return events.filter((e) => {
      if (eventType !== "all" && e.event_type !== eventType) return false;
      if (search.trim()) {
        const q = search.trim().toLowerCase();
        const hay = [e.event_type, e.conversation_id, e.workspace_path, e.model]
          .filter(Boolean)
          .join(" ")
          .toLowerCase();
        if (!hay.includes(q)) return false;
      }
      return true;
    });
  }, [events, eventType, search]);

  const { slice, total, totalPages, page: safePage } = useClientPage(
    filtered,
    page,
    PAGE_SIZE
  );

  const agentMs = tasks.reduce((a, t) => a + t.agent_ms, 0);
  const workMs = dashboard?.task_work_ms ?? 0;

  return (
    <section className="space-y-4">
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="panel p-5 lg:col-span-2">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-lg font-bold">Task activity timeline</h2>
              <p className="tiny mt-1">
                Project-scoped event records · chronological order
              </p>
            </div>
            <span className="pill">Local events</span>
          </div>
          <FilterBar>
            <div className="min-w-[160px] flex-1">
              <label className="label">Search</label>
              <input
                className="field mt-1"
                value={search}
                placeholder="Event, workspace, conversation…"
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
            <div>
              <label className="label">Event type</label>
              <select
                className="field mt-1"
                value={eventType}
                onChange={(e) => setEventType(e.target.value)}
              >
                <option value="all">All types</option>
                {types.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </div>
          </FilterBar>
          <div className="mt-2 space-y-5">
            {slice.length === 0 && <p className="tiny">No events match the current filters.</p>}
            {slice.map((e) => (
              <div key={e.id} className="logrow">
                <div className="flex justify-between gap-3">
                  <strong className="text-sm">{e.event_type}</strong>
                  <span className="tiny">{e.occurred_at}</span>
                </div>
                <div className="tiny mt-1">
                  {[e.conversation_id, e.workspace_path, e.model].filter(Boolean).join(" · ") ||
                    "Metadata recorded"}
                </div>
              </div>
            ))}
          </div>
          <PaginationBar
            page={safePage}
            totalPages={totalPages}
            total={total}
            pageSize={PAGE_SIZE}
            onPage={setPage}
          />
        </div>
        <div className="panel h-fit p-5">
          <h2 className="text-lg font-bold">Time accounting</h2>
          <div className="mt-5 space-y-4">
            <div className="soft p-4">
              <span className="label">Task work time</span>
              <div className="mt-1 text-2xl font-bold">{dur(workMs)}</div>
              <p className="tiny mt-1">Union of work + agent intervals</p>
            </div>
            <div className="soft p-4">
              <span className="label">Agent execution (subset)</span>
              <div className="mt-1 text-2xl font-bold">{dur(agentMs)}</div>
              <p className="tiny mt-1">Do not add to task work time again</p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
