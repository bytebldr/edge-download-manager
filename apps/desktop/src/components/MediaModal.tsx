import React from "react";
import { X, Video } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";
import { backdropVariants, modalVariants } from "../utils/motion";

interface MediaModalProps {
  isOpen: boolean;
  onClose: () => void;
  onGrab?: (quality: string, filename: string) => void;
}

export const MediaModal: React.FC<MediaModalProps> = ({
  isOpen,
  onClose,
  onGrab,
}) => {
  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          variants={backdropVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          className="fixed inset-0 bg-black/75 backdrop-blur-sm flex items-center justify-center p-4 z-50"
          onClick={onClose}
        >
          <motion.div
            variants={modalVariants}
            initial="initial"
            animate="animate"
            exit="exit"
            className="panel-3d w-full max-w-md overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="px-5 py-3 bg-[var(--bg-card)] border-b border-[var(--border-subtle)] flex items-center justify-between">
              <span className="font-bold tracking-tight text-sm flex items-center gap-2">
                <Video className="w-4 h-4 text-cyan-400" />
                Browser Video Stream Grabber
              </span>
              <button
                onClick={onClose}
                className="p-1 rounded-lg text-[var(--text-muted)] hover:text-[var(--text-primary)] cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="p-5 space-y-3 text-xs">
              <div className="p-3 rounded-xl bg-cyan-500/10 border border-cyan-500/20 space-y-1">
                <div className="font-semibold text-cyan-400">Stream Discovered via Native Messaging</div>
                <div className="text-[11px] text-[var(--text-secondary)] font-mono">
                  HLS Master Playlist • 1080p 60fps • No DRM Detected
                </div>
              </div>

              <div>
                <label className="block font-medium text-[var(--text-secondary)] mb-1">Stream Quality</label>
                <select className="w-full px-3 py-2 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-[var(--text-primary)] focus:outline-none cursor-pointer">
                  <option>1080p 60fps (Full HD • ~248 MB)</option>
                  <option>720p 30fps (HD • ~124 MB)</option>
                  <option>Audio Only (AAC 256kbps • ~18 MB)</option>
                </select>
              </div>

              <div>
                <label className="block font-medium text-[var(--text-secondary)] mb-1">Target Filename</label>
                <input
                  type="text"
                  defaultValue="webinar-high-performance-systems-2026.mp4"
                  className="w-full px-3 py-2 rounded-lg bg-[var(--bg-chassis)] border border-[var(--border-subtle)] text-[var(--text-primary)] font-mono"
                />
              </div>
            </div>

            <div className="px-5 py-3 bg-[var(--bg-card)] border-t border-[var(--border-subtle)] flex items-center justify-end gap-2">
              <button
                onClick={onClose}
                className="btn-3d px-4 py-1.5 rounded-lg text-xs font-medium cursor-pointer"
              >
                Dismiss
              </button>
              <button
                onClick={() => {
                  if (onGrab) {
                    onGrab("1080p", "webinar-high-performance-systems-2026.mp4");
                  }
                  onClose();
                }}
                className="btn-primary-3d px-4 py-1.5 rounded-lg text-xs font-semibold cursor-pointer"
              >
                Grab Video Stream
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
};
