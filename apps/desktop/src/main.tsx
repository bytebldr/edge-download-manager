import React, { Component, ErrorInfo, ReactNode } from "react";
import ReactDOM from "react-dom/client";
import { resolveLifecycleRoute, LifecycleRoute } from "./lifecycle";
import { tauriWindowClose } from "./services/tauriBridge";
import "./styles/theme.css";

const App = React.lazy(() => import("./App"));
const I18nProvider = React.lazy(() =>
  import("./features/i18n").then((m) => ({ default: m.I18nProvider }))
);
const LifecycleApp = React.lazy(() =>
  import("./lifecycle").then((m) => ({ default: m.LifecycleApp }))
);
const TrayFlyout = React.lazy(() => import("./components/TrayFlyout"));
const AddDownloadPopup = React.lazy(() => import("./components/AddDownloadPopup"));

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
  const [isTrayFlyout, setIsTrayFlyout] = React.useState<boolean>(() => {
    return typeof window !== "undefined" && window.location.hash === "#tray-flyout";
  });
  const [isAddDownloadPopup, setIsAddDownloadPopup] = React.useState<boolean>(() => {
    return typeof window !== "undefined" && window.location.hash === "#add-download";
  });
  const [lifecycleRoute, setLifecycleRoute] = React.useState<LifecycleRoute | null>(() => {
    return resolveLifecycleRoute();
  });

  React.useEffect(() => {
    const handleHashChange = () => {
      setIsTrayFlyout(window.location.hash === "#tray-flyout");
      setIsAddDownloadPopup(window.location.hash === "#add-download");
      const resolved = resolveLifecycleRoute();
      setLifecycleRoute(resolved);
    };
    window.addEventListener("hashchange", handleHashChange);
    return () => window.removeEventListener("hashchange", handleHashChange);
  }, []);

  if (isTrayFlyout) {
    return (
      <React.Suspense fallback={null}>
        <TrayFlyout />
      </React.Suspense>
    );
  }

  if (isAddDownloadPopup) {
    return (
      <React.Suspense fallback={null}>
        <AddDownloadPopup />
      </React.Suspense>
    );
  }

  if (lifecycleRoute) {
    return (
      <React.Suspense fallback={null}>
        <LifecycleApp
          route={lifecycleRoute}
          onDone={() => {
            tauriWindowClose();
          }}
        />
      </React.Suspense>
    );
  }

  return (
    <React.Suspense fallback={null}>
      <I18nProvider>
        <App />
      </I18nProvider>
    </React.Suspense>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RootErrorBoundary>
      <Root />
    </RootErrorBoundary>
  </React.StrictMode>
);
