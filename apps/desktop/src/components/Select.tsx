import { useState, useRef, useEffect } from "react";
import { ChevronDown, Check } from "lucide-react";

export interface SelectOption<T extends string = string> {
  value: T;
  label: string;
  dotColor?: string;
  hint?: string;
  fontFamily?: string;
}

export interface SelectProps<T extends string = string> {
  value: T;
  onChange: (value: T) => void;
  options: SelectOption<T>[];
  placeholder?: string;
  ariaLabel?: string;
  className?: string;
  disabled?: boolean;
  align?: "left" | "right";
}

/**
 * Custom theme-matched dropdown replacing native HTML <select> elements.
 */
export function Select<T extends string = string>({
  value,
  onChange,
  options,
  placeholder = "Select...",
  ariaLabel,
  className = "",
  disabled = false,
  align = "left",
}: SelectProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    if (!isOpen) return;
    const handleOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsOpen(false);
    };
    document.addEventListener("mousedown", handleOutside);
    window.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleOutside);
      window.removeEventListener("keydown", handleKey);
    };
  }, [isOpen]);

  return (
    <div ref={containerRef} className={`relative inline-block ${className}`}>
      <button
        type="button"
        disabled={disabled}
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        onClick={() => !disabled && setIsOpen((prev) => !prev)}
        className={`w-full h-8 px-2.5 rounded-lg bg-[var(--bg-chassis)] border text-xs text-[var(--text-primary)] flex items-center justify-between gap-2 transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed ${
          isOpen
            ? "border-[var(--accent)] ring-2 ring-[var(--accent)]/20"
            : "border-[var(--border-subtle)] hover:border-[var(--border-strong)]"
        }`}
      >
        <span className="flex items-center gap-2 truncate min-w-0">
          {selected?.dotColor && (
            <span
              className="w-2.5 h-2.5 rounded-full shrink-0"
              style={{ backgroundColor: selected.dotColor }}
            />
          )}
          <span
            className="truncate"
            style={selected?.fontFamily ? { fontFamily: selected.fontFamily } : undefined}
          >
            {selected ? selected.label : placeholder}
          </span>
        </span>
        <ChevronDown
          className={`w-3.5 h-3.5 text-[var(--text-muted)] shrink-0 transition-transform duration-150 ${
            isOpen ? "rotate-180 text-[var(--accent)]" : ""
          }`}
        />
      </button>

      {isOpen && (
        <div
          role="listbox"
          className={`absolute z-50 mt-1.5 min-w-full w-max max-w-[280px] max-h-60 overflow-y-auto rounded-xl bg-[var(--bg-panel)] border border-[var(--border-subtle)] shadow-[0_16px_32px_-8px_rgba(0,0,0,0.65)] py-1 ${
            align === "right" ? "right-0" : "left-0"
          }`}
        >
          {options.map((opt) => {
            const isActive = opt.value === value;
            return (
              <button
                key={opt.value}
                type="button"
                role="option"
                aria-selected={isActive}
                onClick={() => {
                  onChange(opt.value);
                  setIsOpen(false);
                }}
                className={`w-full px-2.5 py-1.5 text-left text-xs flex items-center justify-between gap-3 transition-colors cursor-pointer ${
                  isActive
                    ? "bg-[var(--accent)]/12 text-[var(--text-primary)] font-medium"
                    : "text-[var(--text-secondary)] hover:bg-white/[0.05] hover:text-[var(--text-primary)]"
                }`}
              >
                <span className="flex items-center gap-2 min-w-0">
                  {opt.dotColor && (
                    <span
                      className="w-2.5 h-2.5 rounded-full shrink-0"
                      style={{ backgroundColor: opt.dotColor }}
                    />
                  )}
                  <span
                    className="truncate"
                    style={opt.fontFamily ? { fontFamily: opt.fontFamily } : undefined}
                  >
                    {opt.label}
                  </span>
                </span>
                <span className="flex items-center gap-2 shrink-0">
                  {opt.hint && (
                    <span className="text-[10px] text-[var(--text-muted)]">{opt.hint}</span>
                  )}
                  {isActive && <Check className="w-3.5 h-3.5 text-[var(--accent)] shrink-0" />}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
