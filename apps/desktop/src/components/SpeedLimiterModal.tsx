import React from "react";
import { motion, AnimatePresence } from "motion/react";
import { popoverVariants } from "../utils/motion";

interface SpeedLimiterModalProps {
  isOpen: boolean;
  speedLimit: string;
  onSelect: (val: string) => void;
  onClose: () => void;
}

const SPEED_OPTIONS = [
  "Unlimited",
  "50 MB/s",
  "20 MB/s",
  "10 MB/s",
  "5 MB/s",
  "2 MB/s",
  "500 KB/s",
];

export const SpeedLimiterModal: React.FC<SpeedLimiterModalProps> = ({
  isOpen,
  speedLimit,
  onSelect,
  onClose,
}) => {
  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          variants={popoverVariants}
          initial="initial"
          animate="animate"
          exit="exit"
          className="absolute bottom-full left-0 mb-1.5 w-40 rounded-xl panel-3d shadow-2xl p-1.5 z-50 origin-bottom"
        >
          <div className="text-[10px] uppercase font-bold tracking-wider text-[var(--text-muted)] px-2 py-0.5 mb-1 border-b border-[var(--border-dim)]">
            Speed Throttle
          </div>
          {SPEED_OPTIONS.map((val) => (
            <motion.button
              key={val}
              whileHover={{ x: 2 }}
              whileTap={{ scale: 0.97 }}
              onClick={() => {
                onSelect(val);
                onClose();
              }}
              className={`w-full text-left px-2.5 py-1 text-xs rounded-lg transition-colors cursor-pointer ${
                speedLimit === val
                  ? "bg-[var(--accent)] text-[var(--accent-on)] font-medium"
                  : "text-[var(--text-secondary)] hover:bg-[var(--bg-card-hover)] hover:text-[var(--text-primary)]"
              }`}
            >
              {val}
            </motion.button>
          ))}
        </motion.div>
      )}
    </AnimatePresence>
  );
};
