import React from "react";
import { Cpu, Film, Globe, Layers, Package, Radio, Server, Share2, Sparkles } from "lucide-react";
import { FEATURE_PACKS } from "../../types/featurePacks";

export const PacksSettingsTab: React.FC = () => {
  return (
    <section id="section-packs" className="space-y-5">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <Package className="w-4 h-4 text-[var(--accent)]" />
            <h2 className="text-sm font-semibold text-[var(--text-primary)]">Feature Packs &amp; Protocol Engines</h2>
          </div>
          <p className="text-xs text-[var(--text-muted)] mt-1">
            Multi-protocol adapters, P2P swarm engines, stream demuxers, and accelerated mirror resolvers.
          </p>
        </div>
        <span className="text-xs font-medium text-[var(--text-muted)] shrink-0">
          {FEATURE_PACKS.length} Integrated Packs
        </span>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
        {FEATURE_PACKS.map((pack) => (
          <div
            key={pack.id}
            className="rounded-xl border border-[var(--border-dim)] bg-[var(--bg-card)] p-2 flex items-center justify-between gap-2.5 min-w-0"
          >
            <div className="flex items-center gap-2.5 min-w-0">
              <div className="p-1.5 rounded bg-[var(--bg-chassis)] border border-[var(--border-dim)] shrink-0">
                {pack.id === "http_pack" && <Globe className="w-4 h-4 text-blue-400" />}
                {pack.id === "bittorrent_pack" && <Share2 className="w-4 h-4 text-emerald-400" />}
                {pack.id === "ftp_pack" && <Server className="w-4 h-4 text-amber-400" />}
                {pack.id === "ed2k_pack" && <Radio className="w-4 h-4 text-purple-400" />}
                {pack.id === "m3u8_pack" && <Film className="w-4 h-4 text-cyan-400" />}
                {pack.id === "ffmpeg_pack" && <Layers className="w-4 h-4 text-pink-400" />}
                {pack.id === "yt_dlp_pack" && <Sparkles className="w-4 h-4 text-rose-400" />}
                {pack.id === "github_pack" && <Package className="w-4 h-4 text-slate-300" />}
                {pack.id === "huggingface_pack" && <Cpu className="w-4 h-4 text-yellow-400" />}
              </div>
              <span className="text-xs font-medium text-[var(--text-primary)] truncate" title={pack.name}>
                {pack.name}
              </span>
            </div>
            <span className="shrink-0 font-mono text-[10px] font-medium text-[var(--accent-badge-text)] bg-[var(--accent-badge-bg)] px-1.5 py-0.5 rounded border border-[var(--accent-badge-border)]">
              v{pack.version}
            </span>
          </div>
        ))}
      </div>
    </section>
  );
};
