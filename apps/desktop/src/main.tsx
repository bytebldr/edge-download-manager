import React, { Component, ErrorInfo, ReactNode } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { InstallerWizard } from "./components/InstallerWizard";
import { UninstallerWizard } from "./components/UninstallerWizard";
import "./styles/theme.css";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class RootErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("[EDM Fatal ErrorBoundary]", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            height: "100vh",
            width: "100vw",
            backgroundColor: "#0B0F17",
            color: "#E2E8F0",
            fontFamily: "system-ui, sans-serif",
            padding: "24px",
            boxSizing: "border-box",
            textAlign: "center",
          }}
        >
          <div
            style={{
              padding: "24px",
              maxWidth: "500px",
              backgroundColor: "#161E2E",
              borderRadius: "12px",
              border: "1px solid rgba(255, 255, 255, 0.1)",
              boxShadow: "0 8px 32px rgba(0, 0, 0, 0.4)",
            }}
          >
            <h2 style={{ fontSize: "18px", fontWeight: 600, color: "#F87171", margin: "0 0 12px 0" }}>
              EDM Desktop Render Alert
            </h2>
            <p style={{ fontSize: "13px", color: "#94A3B8", margin: "0 0 16px 0", lineHeight: 1.5 }}>
              {this.state.error?.message || "An unexpected interface error occurred during render."}
            </p>
            <button
              onClick={() => window.location.reload()}
              style={{
                background: "#2563EB",
                color: "#FFFFFF",
                border: "none",
                borderRadius: "6px",
                padding: "8px 16px",
                fontSize: "13px",
                fontWeight: 500,
                cursor: "pointer",
              }}
            >
              Reload EDM
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

function Root() {
  const [installerMode, setInstallerMode] = React.useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    const params = new URLSearchParams(window.location.search);
    return params.get("mode") === "setup" || window.location.hash === "#setup" || params.get("installer") === "1";
  });

  const [uninstallerMode, setUninstallerMode] = React.useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    const params = new URLSearchParams(window.location.search);
    return params.get("mode") === "uninstall" || window.location.hash === "#uninstall" || params.get("uninstall") === "1";
  });

  React.useEffect(() => {
    const handleHashChange = () => {
      if (window.location.hash === "#setup") {
        setInstallerMode(true);
      } else if (window.location.hash === "#uninstall") {
        setUninstallerMode(true);
      }
    };
    window.addEventListener("hashchange", handleHashChange);
    return () => window.removeEventListener("hashchange", handleHashChange);
  }, []);

  if (uninstallerMode) {
    return (
      <UninstallerWizard
        onDone={() => {
          import("./services/tauriBridge").then((m) => m.tauriWindowClose());
        }}
      />
    );
  }

  if (installerMode) {
    return (
      <InstallerWizard
        onDone={() => {
          import("./services/tauriBridge").then((m) => m.tauriWindowClose());
        }}
      />
    );
  }

  return <App />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RootErrorBoundary>
      <Root />
    </RootErrorBoundary>
  </React.StrictMode>
);
