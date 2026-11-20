/**
 * Lifecycle Module Index — Unified exports for EDM setup, update, repair, and uninstall wizards.
 */

export {
  LifecycleApp,
  resolveLifecycleRoute,
  type LifecycleRoute,
  type LifecycleAppProps,
  default,
} from "./LifecycleApp";

export {
  InstallWizard,
  InstallerWizard,
  type InstallWizardProps,
  type InstallerWizardProps,
} from "./wizards/InstallWizard";

export {
  RepairWizard,
  type RepairWizardProps,
} from "./wizards/RepairWizard";

export {
  UninstallWizard,
  UninstallerWizard,
  type UninstallWizardProps,
  type UninstallerWizardProps,
} from "./wizards/UninstallWizard";

export {
  UpdateWizard,
  type UpdateWizardProps,
} from "./wizards/UpdateWizard";
