/**
 * LifecycleApp.tsx — Unified Frameless Container for EDM Lifecycle Operations
 *
 * Manages routing between lifecycle wizards:
 * - #setup / mode=setup -> InstallWizard
 * - #repair / mode=repair -> RepairWizard
 * - #update / mode=update -> UpdateWizard
 * - #uninstall / mode=uninstall -> UninstallWizard
 */

import React, { useState, useEffect, useCallback } from "react";
import { InstallWizard } from "./wizards/InstallWizard";
import { RepairWizard } from "./wizards/RepairWizard";
import { UpdateWizard } from "./wizards/UpdateWizard";
import { UninstallWizard } from "./wizards/UninstallWizard";
import { tauriWindowClose } from "../services/tauriBridge";

export type LifecycleRoute = "setup" | "repair" | "update" | "uninstall";

export interface LifecycleAppProps {
  route?: LifecycleRoute;
  onDone?: () => void;
}

export function resolveLifecycleRoute(): LifecycleRoute | null {
  if (typeof window === "undefined") return null;

  const hash = window.location.hash.toLowerCase();
  if (hash === "#update") return "update";
  if (hash === "#setup") return "setup";
  if (hash === "#repair") return "repair";
  if (hash === "#uninstall") return "uninstall";

  const params = new URLSearchParams(window.location.search);
  const mode = params.get("mode")?.toLowerCase();
  if (mode === "update" || params.get("update") === "1") return "update";
  if (mode === "setup" || params.get("installer") === "1") return "setup";
  if (mode === "repair" || params.get("repair") === "1") return "repair";
  if (mode === "uninstall" || params.get("uninstall") === "1") return "uninstall";

  return null;
}

export const LifecycleApp: React.FC<LifecycleAppProps> = ({
  route: initialRoute,
  onDone,
}) => {
  const [currentRoute, setCurrentRoute] = useState<LifecycleRoute>(() => {
    return initialRoute || resolveLifecycleRoute() || "setup";
  });

  useEffect(() => {
    if (initialRoute) {
      setCurrentRoute(initialRoute);
    }
  }, [initialRoute]);

  useEffect(() => {
    const handleHashChange = () => {
      const resolved = resolveLifecycleRoute();
      if (resolved) {
        setCurrentRoute(resolved);
      }
    };

    window.addEventListener("hashchange", handleHashChange);
    return () => window.removeEventListener("hashchange", handleHashChange);
  }, []);

  const handleDone = useCallback(() => {
    if (onDone) {
      onDone();
    } else {
      try {
        tauriWindowClose();
      } catch {}
    }
  }, [onDone]);

  switch (currentRoute) {
    case "update":
      return <UpdateWizard onDone={handleDone} />;
    case "repair":
      return <RepairWizard onDone={handleDone} />;
    case "uninstall":
      return <UninstallWizard onDone={handleDone} />;
    case "setup":
    default:
      return <InstallWizard onDone={handleDone} />;
  }
};

export default LifecycleApp;
