import React, { useCallback, useEffect, useMemo, useState } from "react";
import { Globe, FolderCog, Plus, Power, Trash2, Check, Search } from "lucide-react";

export interface DomainProxyRule {
  id: string;
  pattern: string;
  proxyUrl: string;
  enabled: boolean;
}

export interface CategoryPolicy {
  category: string;
  maxConcurrent: number | null;
  maxSpeedBps: number | null;
  saveDir: string | null;
}

export interface RoutingPoliciesPayload {
  domainProxyRules: DomainProxyRule[];
  categoryPolicies: CategoryPolicy[];
}

const DEFAULT_CATEGORIES: CategoryPolicy[] = [
  "Video",
  "Music",
  "Documents",
  "Programs",
  "Compressed",
  "Other",
].map((category) => ({
  category,
  maxConcurrent: null,
  maxSpeedBps: null,
  saveDir: null,
}));

const STORAGE_KEY = "edm_routing_policies";

const isTauriRuntime = (): boolean =>
  typeof window !== "undefined" && ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

function readLocalPolicies(): RoutingPoliciesPayload {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<RoutingPoliciesPayload>;
      return {
        domainProxyRules: Array.isArray(parsed.domainProxyRules) ? parsed.domainProxyRules : [],
        categoryPolicies:
          Array.isArray(parsed.categoryPolicies) && parsed.categoryPolicies.length > 0
            ? parsed.categoryPolicies
            : DEFAULT_CATEGORIES,
      };
    }
  } catch {
    // Ignore corrupt localStorage
  }
  return { domainProxyRules: [], categoryPolicies: DEFAULT_CATEGORIES };
}

async function invokeRouting<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  const current = readLocalPolicies();
  if (cmd === "get_routing_policies") {
    return current as unknown as T;
  }
  if (cmd === "save_domain_proxy_rules") {
    const rules = (args?.rules as DomainProxyRule[]) ?? [];
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...current, domainProxyRules: rules }));
    return rules as unknown as T;
  }
  if (cmd === "save_category_policies") {
    const policies = (args?.policies as CategoryPolicy[]) ?? [];
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...current, categoryPolicies: policies }));
    return policies as unknown as T;
  }
  throw new Error(`Unsupported command: ${cmd}`);
}

function matchesDomainPattern(host: string, pattern: string): boolean {
  const h = host.trim().replace(/\.$/, "").toLowerCase();
  const p = pattern.trim().replace(/\.$/, "").toLowerCase();
  if (!h || !p) return false;
  if (p === "*") return true;
  if (p.startsWith("*.")) {
    const suffix = p.slice(2);
    return h.length > suffix.length + 1 && h.endsWith(`.${suffix}`);
  }
  const regex = new RegExp(`^${p.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*")}$`);
  return regex.test(h);
}

export const RoutingAndCategoryPanel: React.FC = () => {
  const [rules, setRules] = useState<DomainProxyRule[]>([]);
  const [policies, setPolicies] = useState<CategoryPolicy[]>(DEFAULT_CATEGORIES);
  const [newPattern, setNewPattern] = useState("");
  const [newProxyUrl, setNewProxyUrl] = useState("socks5://127.0.0.1:1080");
  const [testUrl, setTestUrl] = useState("https://api.github.com/repos/org/app");
  const [statusMsg, setStatusMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await invokeRouting<RoutingPoliciesPayload>("get_routing_policies");
      setRules(data.domainProxyRules);
      setPolicies(data.categoryPolicies);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const matchedRule = useMemo(() => {
    try {
      const host = new URL(testUrl.trim()).hostname;
      return rules.find((r) => r.enabled && matchesDomainPattern(host, r.pattern)) ?? null;
    } catch {
      return null;
    }
  }, [rules, testUrl]);

  const persistRules = async (next: DomainProxyRule[], message?: string) => {
    try {
      const saved = await invokeRouting<DomainProxyRule[]>("save_domain_proxy_rules", {
        rules: next,
      });
      setRules(saved);
      setError(null);
      if (message) setStatusMsg(message);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleAddRule = async (e: React.FormEvent) => {
    e.preventDefault();
    const pattern = newPattern.trim();
    const proxyUrl = newProxyUrl.trim();
    if (!pattern || !proxyUrl) {
      setError("Both domain pattern and proxy URL are required.");
      return;
    }
    const id =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `rule-${Date.now()}`;
    await persistRules([...rules, { id, pattern, proxyUrl, enabled: true }], "Rule added.");
    setNewPattern("");
  };

  const handleSavePolicies = async () => {
    try {
      const saved = await invokeRouting<CategoryPolicy[]>("save_category_policies", { policies });
      setPolicies(saved);
      setError(null);
      setStatusMsg("Category policies saved.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const updatePolicy = (index: number, patch: Partial<CategoryPolicy>) => {
    setPolicies((prev) => prev.map((p, i) => (i === index ? { ...p, ...patch } : p)));
  };

  return (
    <div className="flex flex-col gap-5 p-4 rounded-2xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)]">
      {error && (
        <div className="px-3 py-2 rounded-lg text-xs border border-[var(--danger)]/40 bg-[var(--danger)]/10 text-[var(--danger)]">
          {error}
        </div>
      )}
      {statusMsg && !error && (
        <div className="px-3 py-2 rounded-lg text-xs border border-[var(--accent)]/40 bg-[var(--accent)]/10 text-[var(--accent)]">
          {statusMsg}
        </div>
      )}

      {/* Per-Domain Split Proxy Routing */}
      <section className="flex flex-col gap-3">
        <div className="flex items-center gap-2.5">
          <Globe className="w-5 h-5 text-[var(--accent)]" />
          <div>
            <h2 className="text-sm font-semibold tracking-tight">Per-Domain Proxy Routing</h2>
            <p className="text-xs text-[var(--text-muted)]">
              Route matching hostnames (e.g. *.github.com, huggingface.co) through specific proxies.
            </p>
          </div>
        </div>

        <form onSubmit={handleAddRule} className="grid grid-cols-1 md:grid-cols-5 gap-2">
          <input
            type="text"
            value={newPattern}
            onChange={(e) => setNewPattern(e.target.value)}
            placeholder="*.github.com"
            className="md:col-span-2 px-3 py-1.5 rounded-lg text-xs bg-[var(--bg-card)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
          />
          <input
            type="text"
            value={newProxyUrl}
            onChange={(e) => setNewProxyUrl(e.target.value)}
            placeholder="socks5://127.0.0.1:1080"
            className="md:col-span-2 px-3 py-1.5 rounded-lg text-xs bg-[var(--bg-card)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
          />
          <button
            type="submit"
            className="inline-flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-[var(--accent)] text-[var(--accent-on)] hover:bg-[var(--accent-hover)] transition-colors cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            Add Rule
          </button>
        </form>

        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-[var(--bg-card)] border border-[var(--border-subtle)] text-xs">
          <Search className="w-3.5 h-3.5 text-[var(--text-muted)] shrink-0" />
          <input
            type="text"
            value={testUrl}
            onChange={(e) => setTestUrl(e.target.value)}
            placeholder="Test URL against rules..."
            className="flex-1 bg-transparent outline-none text-[var(--text-primary)]"
          />
          <span className="text-[var(--text-muted)]">
            {matchedRule ? `Matches ${matchedRule.pattern} -> ${matchedRule.proxyUrl}` : "Direct"}
          </span>
        </div>

        <div className="flex flex-col gap-2">
          {rules.length === 0 ? (
            <div className="text-xs text-[var(--text-muted)] py-2">
              No domain proxy rules configured. All downloads connect directly.
            </div>
          ) : (
            rules.map((rule) => (
              <div
                key={rule.id}
                className="flex items-center justify-between gap-3 px-3 py-2 rounded-xl bg-[var(--bg-card)] border border-[var(--border-subtle)] text-xs"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <span className="font-mono font-medium truncate">{rule.pattern}</span>
                  <span className="text-[var(--text-muted)]">-&gt;</span>
                  <span className="font-mono text-[var(--text-secondary)] truncate">
                    {rule.proxyUrl}
                  </span>
                </div>
                <div className="flex items-center gap-1.5 shrink-0">
                  <button
                    type="button"
                    onClick={() =>
                      persistRules(
                        rules.map((r) => (r.id === rule.id ? { ...r, enabled: !r.enabled } : r))
                      )
                    }
                    className={`p-1.5 rounded-md border transition-colors cursor-pointer ${
                      rule.enabled
                        ? "border-[var(--accent)]/40 text-[var(--accent)] bg-[var(--accent)]/10"
                        : "border-[var(--border-subtle)] text-[var(--text-muted)]"
                    }`}
                    title={rule.enabled ? "Disable rule" : "Enable rule"}
                  >
                    <Power className="w-3.5 h-3.5" />
                  </button>
                  <button
                    type="button"
                    onClick={() => persistRules(rules.filter((r) => r.id !== rule.id), "Rule removed.")}
                    className="p-1.5 rounded-md text-[var(--text-muted)] hover:text-[var(--danger)] cursor-pointer"
                    title="Delete rule"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
      </section>

      {/* Per-Category Concurrency & Speed Caps */}
      <section className="flex flex-col gap-3 pt-2 border-t border-[var(--border-subtle)]">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2.5">
            <FolderCog className="w-5 h-5 text-[var(--accent)]" />
            <div>
              <h2 className="text-sm font-semibold tracking-tight">Category Policies</h2>
              <p className="text-xs text-[var(--text-muted)]">
                Set per-category concurrency caps, bandwidth limits (MB/s), and save directories.
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={handleSavePolicies}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-[var(--accent)] text-[var(--accent-on)] hover:bg-[var(--accent-hover)] transition-colors cursor-pointer"
          >
            <Check className="w-3.5 h-3.5" />
            Save Policies
          </button>
        </div>

        <div className="flex flex-col gap-2">
          {policies.map((policy, idx) => (
            <div
              key={policy.category}
              className="grid grid-cols-1 md:grid-cols-4 gap-2 items-center px-3 py-2 rounded-xl bg-[var(--bg-card)] border border-[var(--border-subtle)] text-xs"
            >
              <span className="font-medium">{policy.category}</span>
              <label className="flex items-center gap-1.5 text-[var(--text-muted)]">
                <span>Max</span>
                <input
                  type="number"
                  min={1}
                  placeholder="Auto"
                  value={policy.maxConcurrent ?? ""}
                  onChange={(e) => {
                    const val = e.target.value.trim();
                    updatePolicy(idx, {
                      maxConcurrent: val === "" ? null : Math.max(1, Number(val)),
                    });
                  }}
                  className="w-20 px-2 py-1 rounded bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
                />
              </label>
              <label className="flex items-center gap-1.5 text-[var(--text-muted)]">
                <span>MB/s</span>
                <input
                  type="number"
                  min={0}
                  step="0.5"
                  placeholder="Unlimited"
                  value={
                    policy.maxSpeedBps !== null
                      ? Number((policy.maxSpeedBps / (1024 * 1024)).toFixed(2))
                      : ""
                  }
                  onChange={(e) => {
                    const val = e.target.value.trim();
                    const num = Number(val);
                    updatePolicy(idx, {
                      maxSpeedBps:
                        val === "" || !Number.isFinite(num) || num <= 0
                          ? null
                          : Math.round(num * 1024 * 1024),
                    });
                  }}
                  className="w-24 px-2 py-1 rounded bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
                />
              </label>
              <input
                type="text"
                placeholder="Default folder"
                value={policy.saveDir ?? ""}
                onChange={(e) =>
                  updatePolicy(idx, { saveDir: e.target.value.trim() || null })
                }
                className="px-2 py-1 rounded bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)]"
              />
            </div>
          ))}
        </div>
      </section>
    </div>
  );
};
