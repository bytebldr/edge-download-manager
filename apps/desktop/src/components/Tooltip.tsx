import React from "react";

export interface TooltipProps {
  content: string;
  side?: "top" | "bottom" | "left" | "right";
  align?: "center" | "start" | "end";
  children: React.ReactNode;
  className?: string;
}

const getPositionClasses = (
  side: NonNullable<TooltipProps["side"]>,
  align: NonNullable<TooltipProps["align"]>
): string => {
  if (side === "top" || side === "bottom") {
    const v = side === "top" ? "bottom-full mb-2" : "top-full mt-2";
    const h =
      align === "start"
        ? "left-0"
        : align === "end"
          ? "right-0"
          : "left-1/2 -translate-x-1/2";
    return `${v} ${h}`;
  }
  const h = side === "left" ? "right-full mr-2" : "left-full ml-2.5";
  const v =
    align === "start"
      ? "top-0"
      : align === "end"
        ? "bottom-0"
        : "top-1/2 -translate-y-1/2";
  return `${h} ${v}`;
};

/**
 * Custom theme-matched UI tooltip replacing generic browser title popups.
 */
export const Tooltip: React.FC<TooltipProps> = ({
  content,
  side = "top",
  align = "center",
  children,
  className = "",
}) => {
  return (
    <div className={`relative inline-flex items-center justify-center group/tooltip ${className}`}>
      {children}
      <div
        role="tooltip"
        className={`pointer-events-none absolute z-50 px-2.5 py-1 rounded-lg bg-[var(--bg-panel)] border border-[var(--border-subtle)] text-[var(--text-primary)] text-[11px] font-medium whitespace-nowrap shadow-[0_8px_20px_-4px_rgba(0,0,0,0.45)] opacity-0 scale-95 group-hover/tooltip:opacity-100 group-hover/tooltip:scale-100 transition-[opacity,transform] duration-150 ease-out ${getPositionClasses(side, align)}`}
      >
        {content}
      </div>
    </div>
  );
};
