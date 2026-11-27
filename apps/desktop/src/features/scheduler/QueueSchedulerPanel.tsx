import React, { useCallback, useEffect, useState } from "react";
import { Tooltip } from "../../components/Tooltip";
import { Select } from "../../components/Select";
import { CalendarClock, Gauge, Layers, Plus, Power, Trash2, Check, X } from "lucide-react";

export type QueuePostAction = "none" | "shutdown" | "sleep" | "hibernate" | "exit" | "notify";

export const POST_ACTION_OPTIONS: Array<{ value: QueuePostAction; label: string }> = [
  { value: "none", label: "None (Keep running)" },
  { value: "shutdown", label: "Shutdown Computer" },
  { value: "sleep", label: "Sleep Computer" },
  { value: "hibernate", label: "Hibernate Computer" },
  { value: "exit", label: "Exit EDM" },
  { value: "notify", label: "Play Sound & Notify" },
];

export interface ScheduleWindowConfig {
  enabled: boolean;
  startHhmm: string;
  endHhmm: string;
  daysOfWeek: number[];
}

export interface ScheduledQueueConfig {
  id: string;
  name: string;
  maxConcurrent: number;
  maxSpeedBps: number | null;
  taskIds: string[];
  enabled: boolean;
  schedule: ScheduleWindowConfig;
  postAction?: QueuePostAction;
  autoStartOnWindow?: boolean;
  pauseOnWindowClose?: boolean;
  autoRetryFailed?: boolean;
}

export interface QueueScheduleEvaluation {
  id: string;
  name: string;
  enabled: boolean;
  withinWindow: boolean;
  activeAllowed: boolean;
  maxConcurrent: number;
  maxSpeedBps: number | null;
}

const DAY_LABELS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] as const;
const STORAGE_KEY = "edm_queues";

const isTauriRuntime = (): boolean =>
  typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

async function invokeScheduler<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const raw = localStorage.getItem(STORAGE_KEY);
  const current: ScheduledQueueConfig[] = raw ? JSON.parse(raw) : [];

  if (cmd === "list_queues") {
    return current as unknown as T;
  }
  if (cmd === "save_queue") {
    const incoming = args?.queue as ScheduledQueueConfig;
    const idx = current.findIndex((q) => q.id === incoming.id);
    const next =
      idx >= 0
        ? current.map((q, i) => (i === idx ? incoming : q))
        : [...current, incoming];
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    return next as unknown as T;
  }
  if (cmd === "delete_queue") {
    const queueId = args?.queueId as string;
    const next = current.filter((q) => q.id !== queueId);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    return next as unknown as T;
  }
  if (cmd === "evaluate_queue_schedules") {
    const now = new Date();
    const nowMins = now.getHours() * 60 + now.getMinutes();
    const today = now.getDay();
    const evals: QueueScheduleEvaluation[] = current.map((q) => {
      const withinWindow = evaluateWindowLocal(q.schedule, nowMins, today);
      return {
        id: q.id,
        name: q.name,
        enabled: q.enabled,
        withinWindow,
        activeAllowed: q.enabled && withinWindow,
        maxConcurrent: q.maxConcurrent,
        maxSpeedBps: q.maxSpeedBps,
      };
    });
    return evals as unknown as T;
  }
  throw new Error(`Unsupported command: ${cmd}`);
}

function evaluateWindowLocal(win: ScheduleWindowConfig, nowMins: number, today: number): boolean {
  if (!win.enabled) return true;
  const parse = (hhmm: string): number | null => {
    const [h, m] = hhmm.split(":").map(Number);
    if (!Number.isInteger(h) || !Number.isInteger(m) || h < 0 || h > 23 || m < 0 || m > 59) {
      return null;
    }
    return h * 60 + m;
  };
  const start = parse(win.startHhmm);
  const end = parse(win.endHhmm);
  if (start === null || end === null) return false;
  const dayAllowed = (d: number) => win.daysOfWeek.length === 0 || win.daysOfWeek.includes(d);
  if (start === end) return dayAllowed(today);
  if (start < end) return dayAllowed(today) && nowMins >= start && nowMins < end;
  if (nowMins >= start) return dayAllowed(today);
  if (nowMins < end) return dayAllowed((today + 6) % 7);
  return false;
}

function formatSpeedCap(bps: number | null): string {
  if (bps === null || bps <= 0) return "Unlimited";
  if (bps >= 1024 * 1024) return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
  return `${Math.round(bps / 1024)} KB/s`;
}

function makeBlankQueue(): ScheduledQueueConfig {
  const id =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `queue-${Date.now()}`;
  return {
    id,
    name: "",
    maxConcurrent: 3,
    maxSpeedBps: null,
    taskIds: [],
    enabled: true,
    schedule: {
      enabled: true,
      startHhmm: "23:00",
      endHhmm: "06:00",
      daysOfWeek: [0, 1, 2, 3, 4, 5, 6],
    },
    postAction: "none",
    autoStartOnWindow: true,
    pauseOnWindowClose: true,
    autoRetryFailed: false,
  };
}

export const QueueSchedulerPanel: React.FC = () => {
  const [queues, setQueues] = useState<ScheduledQueueConfig[]>([]);
  const [evaluations, setEvaluations] = useState<Record<string, QueueScheduleEvaluation>>({});
  const [editing, setEditing] = useState<ScheduledQueueConfig | null>(null);
  const [speedMbInput, setSpeedMbInput] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [list, evals] = await Promise.all([
        invokeScheduler<ScheduledQueueConfig[]>("list_queues"),
        invokeScheduler<QueueScheduleEvaluation[]>("evaluate_queue_schedules"),
      ]);
      setQueues(list);
      const map: Record<string, QueueScheduleEvaluation> = {};
      for (const item of evals) map[item.id] = item;
      setEvaluations(map);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const startEdit = (queue: ScheduledQueueConfig) => {
    setEditing({
      ...queue,
      schedule: { ...queue.schedule, daysOfWeek: [...queue.schedule.daysOfWeek] },
    });
    setSpeedMbInput(
      queue.maxSpeedBps && queue.maxSpeedBps > 0
        ? String(Number((queue.maxSpeedBps / (1024 * 1024)).toFixed(2)))
        : ""
    );
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editing) return;
    const trimmed = editing.name.trim();
    if (!trimmed) {
      setError("Queue name is required.");
      return;
    }
    const mbVal = Number(speedMbInput);
    const maxSpeedBps =
      speedMbInput.trim() !== "" && Number.isFinite(mbVal) && mbVal > 0
        ? Math.round(mbVal * 1024 * 1024)
        : null;

    const payload: ScheduledQueueConfig = {
      ...editing,
      name: trimmed,
      maxConcurrent: Math.max(1, Math.floor(editing.maxConcurrent || 1)),
      maxSpeedBps,
    };

    try {
      await invokeScheduler<ScheduledQueueConfig[]>("save_queue", { queue: payload });
      setEditing(null);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleDelete = async (queueId: string) => {
    try {
      await invokeScheduler<ScheduledQueueConfig[]>("delete_queue", { queueId });
      if (editing?.id === queueId) setEditing(null);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleToggleEnabled = async (queue: ScheduledQueueConfig) => {
    try {
      await invokeScheduler<ScheduledQueueConfig[]>("save_queue", {
        queue: { ...queue, enabled: !queue.enabled },
      });
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const toggleDay = (day: number) => {
    if (!editing) return;
    const exists = editing.schedule.daysOfWeek.includes(day);
    const nextDays = exists
      ? editing.schedule.daysOfWeek.filter((d) => d !== day)
      : [...editing.schedule.daysOfWeek, day].sort((a, b) => a - b);
    setEditing({
      ...editing,
      schedule: { ...editing.schedule, daysOfWeek: nextDays },
    });
  };

  return (
    <div className="flex flex-col gap-4 text-[var(--text-primary)]">
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs text-[var(--text-muted)]">
          Configure named queues, concurrency caps, bandwidth limits, and active time windows.
        </p>
        <button
          type="button"
          onClick={() => startEdit(makeBlankQueue())}
          className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--accent)] hover:bg-white/[0.08] hover:border-[var(--accent)]/40 inline-flex items-center gap-1.5 shrink-0 transition-colors cursor-pointer"
        >
          <Plus className="w-3.5 h-3.5 stroke-[2]" />
          <span>New Queue</span>
        </button>
      </div>

      {error && (
        <div className="px-3 py-2 rounded-lg text-xs border border-[var(--danger)]/40 bg-[var(--danger)]/10 text-[var(--danger)]">
          {error}
        </div>
      )}

      {editing && (
        <form
          onSubmit={handleSave}
          className="flex flex-col gap-3.5 p-4 rounded-xl bg-[var(--bg-chassis)] border border-[var(--border-subtle)]"
        >
          <div className="flex items-center justify-between">
            <span className="text-xs font-semibold text-[var(--text-primary)]">
              {queues.some((q) => q.id === editing.id) ? "Edit queue" : "Create queue"}
            </span>
            <button
              type="button"
              onClick={() => setEditing(null)}
              className="w-7 h-7 rounded-lg flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-white/[0.06] transition-colors cursor-pointer"
              aria-label="Cancel editing"
            >
              <X className="w-4 h-4 stroke-[1.75]" />
            </button>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <label className="flex flex-col gap-1 text-xs">
              <span className="text-[var(--text-secondary)] font-medium">Queue name</span>
              <input
                type="text"
                value={editing.name}
                onChange={(e) => setEditing({ ...editing, name: e.target.value })}
                placeholder="e.g. Overnight Bulk"
                className="h-8 px-2.5 rounded-lg bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-xs text-[var(--text-primary)] placeholder-[var(--text-muted)] focus:outline-none focus:border-[var(--accent)] transition-colors"
              />
            </label>

            <label className="flex flex-col gap-1 text-xs">
              <span className="text-[var(--text-secondary)] font-medium">Max concurrent</span>
              <input
                type="number"
                min={1}
                max={32}
                value={editing.maxConcurrent}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    maxConcurrent: Math.max(1, Number(e.target.value) || 1),
                  })
                }
                className="h-8 px-2.5 rounded-lg bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-xs text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)] transition-colors"
              />
            </label>

            <label className="flex flex-col gap-1 text-xs">
              <span className="text-[var(--text-secondary)]">Speed Cap (MB/s, blank = ∞)</span>
              <input
                type="number"
                min={0}
                step="0.5"
                value={speedMbInput}
                onChange={(e) => setSpeedMbInput(e.target.value)}
                placeholder="Unlimited"
                className="px-2.5 py-1.5 rounded-lg bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
              />
            </label>
          </div>

          <div className="flex flex-col gap-2 pt-2 border-t border-[var(--border-dim)]">
            <label className="inline-flex items-center gap-2 text-xs cursor-pointer select-none">
              <input
                type="checkbox"
                checked={editing.schedule.enabled}
                onChange={(e) =>
                  setEditing({
                    ...editing,
                    schedule: { ...editing.schedule, enabled: e.target.checked },
                  })
                }
                className="accent-[var(--accent)]"
              />
              <span className="font-medium text-[var(--text-primary)]">
                Restrict to schedule window
              </span>
            </label>

            {editing.schedule.enabled && (
              <div className="flex flex-col gap-2.5 pl-5">
                <div className="flex flex-wrap items-center gap-3 text-xs">
                  <label className="flex items-center gap-1.5">
                    <span className="text-[var(--text-secondary)]">Start</span>
                    <input
                      type="time"
                      value={editing.schedule.startHhmm}
                      onChange={(e) =>
                        setEditing({
                          ...editing,
                          schedule: { ...editing.schedule, startHhmm: e.target.value },
                        })
                      }
                      className="px-2 py-1 rounded-md bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
                    />
                  </label>
                  <label className="flex items-center gap-1.5">
                    <span className="text-[var(--text-secondary)]">Stop</span>
                    <input
                      type="time"
                      value={editing.schedule.endHhmm}
                      onChange={(e) =>
                        setEditing({
                          ...editing,
                          schedule: { ...editing.schedule, endHhmm: e.target.value },
                        })
                      }
                      className="px-2 py-1 rounded-md bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
                    />
                  </label>
                </div>

                <div className="flex flex-wrap items-center gap-1.5">
                  {DAY_LABELS.map((label, idx) => {
                    const active = editing.schedule.daysOfWeek.includes(idx);
                    return (
                      <button
                        key={label}
                        type="button"
                        onClick={() => toggleDay(idx)}
                        className={`px-2 py-1 rounded-md text-[11px] font-medium border transition-colors cursor-pointer ${
                          active
                            ? "bg-[var(--accent-badge-bg)] border-[var(--accent-badge-border)] text-[var(--accent-badge-text)]"
                            : "bg-[var(--bg-panel)] border-[var(--border-dim)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                        }`}
                      >
                        {label}
                      </button>
                    );
                  })}
                </div>
              </div>
            )}
          </div>

          {/* Post-Completion Action & Queue Behavior */}
          <div className="flex flex-col gap-3 pt-3 border-t border-[var(--border-dim)]">
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 items-center">
              <div>
                <span className="text-xs font-medium text-[var(--text-primary)] block">
                  On Queue Complete
                </span>
                <span className="text-[11px] text-[var(--text-muted)]">
                  System action once every item in this queue finishes.
                </span>
              </div>
              <Select
                value={(editing.postAction || "none") as QueuePostAction}
                onChange={(val) =>
                  setEditing({
                    ...editing,
                    postAction: val as QueuePostAction,
                  })
                }
                ariaLabel="Action on queue complete"
                align="right"
                className="w-full"
                options={POST_ACTION_OPTIONS}
              />
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-2 text-[11px]">
              <label className="inline-flex items-center gap-2 cursor-pointer select-none text-[var(--text-secondary)] hover:text-[var(--text-primary)]">
                <input
                  type="checkbox"
                  checked={editing.autoStartOnWindow ?? true}
                  onChange={(e) =>
                    setEditing({ ...editing, autoStartOnWindow: e.target.checked })
                  }
                  className="edm-checkbox"
                />
                <span>Auto-start on window</span>
              </label>

              <label className="inline-flex items-center gap-2 cursor-pointer select-none text-[var(--text-secondary)] hover:text-[var(--text-primary)]">
                <input
                  type="checkbox"
                  checked={editing.pauseOnWindowClose ?? true}
                  onChange={(e) =>
                    setEditing({ ...editing, pauseOnWindowClose: e.target.checked })
                  }
                  className="edm-checkbox"
                />
                <span>Pause on window end</span>
              </label>

              <label className="inline-flex items-center gap-2 cursor-pointer select-none text-[var(--text-secondary)] hover:text-[var(--text-primary)]">
                <input
                  type="checkbox"
                  checked={editing.autoRetryFailed ?? false}
                  onChange={(e) =>
                    setEditing({ ...editing, autoRetryFailed: e.target.checked })
                  }
                  className="edm-checkbox"
                />
                <span>Auto-retry failed</span>
              </label>
            </div>
          </div>

          <div className="flex justify-end gap-2 pt-2 border-t border-[var(--border-dim)]">
            <button
              type="button"
              onClick={() => setEditing(null)}
              className="h-8 px-3 rounded-lg bg-white/[0.04] border border-[var(--border-subtle)] text-xs font-medium text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-white/[0.08] cursor-pointer transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              className="inline-flex items-center gap-1.5 h-8 px-3.5 rounded-lg text-xs font-medium bg-[var(--btn-contrast-bg)] text-[var(--btn-contrast-text)] hover:bg-[var(--btn-contrast-hover)] cursor-pointer transition-colors"
            >
              <Check className="w-3.5 h-3.5" />
              Save Queue
            </button>
          </div>
        </form>
      )}

      {loading ? (
        <div className="py-6 text-center text-xs text-[var(--text-muted)]">Loading queues…</div>
      ) : queues.length === 0 ? (
        <div className="py-8 text-center text-xs text-[var(--text-muted)] border-t border-[var(--border-dim)]">
          No scheduled queues configured yet. Create one to manage concurrency and time windows.
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {queues.map((q) => {
            const evalInfo = evaluations[q.id];
            const activeNow = evalInfo?.activeAllowed ?? q.enabled;
            const daysText =
              q.schedule.daysOfWeek.length === 0 || q.schedule.daysOfWeek.length === 7
                ? "Every day"
                : q.schedule.daysOfWeek.map((d) => DAY_LABELS[d]).join(", ");

            return (
              <div
                key={q.id}
                className="flex items-center justify-between gap-3 p-3 rounded-xl bg-[var(--bg-card)] border border-[var(--border-dim)] hover:border-[var(--border-subtle)] transition-colors"
              >
                <div className="flex flex-col gap-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium truncate">{q.name}</span>
                    <span
                      className={`px-1.5 py-0.5 rounded text-[10px] font-semibold uppercase tracking-wide ${
                        !q.enabled
                          ? "bg-[var(--border-dim)] text-[var(--text-muted)]"
                          : activeNow
                            ? "bg-[var(--success)]/15 text-[var(--success)]"
                            : "bg-[var(--warning)]/15 text-[var(--warning)]"
                      }`}
                    >
                      {!q.enabled ? "Disabled" : activeNow ? "Active Window" : "Outside Window"}
                    </span>
                  </div>
                  <div className="flex flex-wrap items-center gap-3 text-xs text-[var(--text-secondary)]">
                    <span className="inline-flex items-center gap-1">
                      <Layers className="w-3.5 h-3.5 text-[var(--text-muted)]" />
                      Max {q.maxConcurrent} concurrent
                    </span>
                    <span className="inline-flex items-center gap-1">
                      <Gauge className="w-3.5 h-3.5 text-[var(--text-muted)]" />
                      {formatSpeedCap(q.maxSpeedBps)}
                    </span>
                    <span className="inline-flex items-center gap-1">
                      <CalendarClock className="w-3.5 h-3.5 text-[var(--text-muted)]" />
                      {q.schedule.enabled
                        ? `${q.schedule.startHhmm}–${q.schedule.endHhmm} (${daysText})`
                        : "Always allowed"}
                    </span>
                  </div>
                </div>

                <div className="flex items-center gap-1.5 shrink-0">
                  <Tooltip content={q.enabled ? "Disable queue" : "Enable queue"} side="top">
                    <button
                      type="button"
                      onClick={() => handleToggleEnabled(q)}
                      aria-label={q.enabled ? "Disable queue" : "Enable queue"}
                      className={`p-1.5 rounded-lg border transition-colors cursor-pointer ${
                        q.enabled
                          ? "border-[var(--accent-badge-border)] bg-[var(--accent-badge-bg)] text-[var(--accent-badge-text)]"
                          : "border-[var(--border-dim)] text-[var(--text-muted)] hover:text-[var(--text-primary)]"
                      }`}
                    >
                      <Power className="w-3.5 h-3.5" />
                    </button>
                  </Tooltip>
                  <button
                    type="button"
                    onClick={() => startEdit(q)}
                    className="px-2.5 py-1 rounded-lg text-xs border border-[var(--border-subtle)] text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-card-hover)] cursor-pointer"
                  >
                    Edit
                  </button>
                  <Tooltip content="Delete queue" side="top" align="end">
                    <button
                      type="button"
                      onClick={() => handleDelete(q.id)}
                      aria-label="Delete queue"
                      className="p-1.5 rounded-lg text-[var(--text-muted)] hover:text-[var(--danger)] hover:bg-[var(--danger)]/10 cursor-pointer"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </Tooltip>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
