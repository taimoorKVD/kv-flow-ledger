import { useEffect, useState } from "react";
import { api, type ActivityEvent } from "@/lib/api";
import { dur } from "@/lib/format";
import { useAppStore } from "@/stores/appStore";

export default function ActivityPage() {
  const selectedProjectId = useAppStore((s) => s.selectedProjectId);
  const dashboard = useAppStore((s) => s.dashboard);
  const tasks = useAppStore((s) => s.tasks);
  const [events, setEvents] = useState<ActivityEvent[]>([]);

  useEffect(() => {
    void api
      .listActivity(selectedProjectId ?? undefined)
      .then(setEvents)
      .catch(() => setEvents([]));
  }, [selectedProjectId]);

  const agentMs = tasks.reduce((a, t) => a + t.agent_ms, 0);
  const workMs = dashboard?.task_work_ms ?? 0;

  return (
    <section className="space-y-4">
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="panel p-5 lg:col-span-2">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-lg font-bold">Task activity timeline</h2>
              <p className="tiny mt-1">Event-level records · chronological order</p>
            </div>
            <span className="pill">Local events</span>
          </div>
          <div className="mt-6 space-y-5">
            {events.length === 0 && <p className="tiny">No events recorded yet.</p>}
            {events.map((e) => (
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
