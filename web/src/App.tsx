import { useState, useEffect } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { AuthWizard } from "./components/AuthWizard";
import { Dashboard } from "./components/Dashboard";
import { ErrorBoundary } from "./components/ErrorBoundary";
import * as api from "./lib/api";
import "./App.css";

import { Toaster } from "sonner";
import { ConfirmProvider } from "./context/ConfirmContext";
import { ThemeProvider, useTheme } from "./context/ThemeContext";
import { DropZoneProvider } from "./contexts/DropZoneContext";

const queryClient = new QueryClient();

function AppContent() {
  const [authState, setAuthState] = useState<"loading" | "login" | "telegram" | "ready">("loading");
  const { theme } = useTheme();

  const checkAuth = async () => {
    try {
      const status = await api.authStatus();
      if (!status.authenticated) {
        setAuthState("login");
        return;
      }
      const tgStatus = await api.telegramStatus();
      if (tgStatus.connected) {
        setAuthState("ready");
      } else {
        setAuthState("telegram");
      }
    } catch {
      setAuthState("login");
    }
  };

  useEffect(() => {
    checkAuth();
  }, []);

  const handleLogin = () => {
    checkAuth();
  };

  const handleLogout = async () => {
    try { await api.logout(); } catch { /* ignore */ }
    queryClient.clear();
    setAuthState("login");
  };

  if (authState === "loading") {
    return (
      <main className="h-screen w-screen flex items-center justify-center bg-telegram-bg text-telegram-text">
        <div className="flex flex-col items-center gap-4">
          <div className="w-10 h-10 border-4 border-telegram-primary border-t-transparent rounded-full animate-spin" />
          <p className="text-sm text-telegram-subtext">Loading...</p>
        </div>
      </main>
    );
  }

  return (
    <main className="h-screen w-screen text-telegram-text overflow-hidden selection:bg-telegram-primary/30 relative">
      <Toaster theme={theme} position="bottom-center" />
      {authState === "ready" ? (
        <Dashboard onLogout={handleLogout} />
      ) : (
        <AuthWizard onLogin={handleLogin} />
      )}
    </main>
  );
}


function App() {
  return (
    <ErrorBoundary>
      <ThemeProvider>
        <QueryClientProvider client={queryClient}>
          <ConfirmProvider>
            <DropZoneProvider>
              <AppContent />
            </DropZoneProvider>
          </ConfirmProvider>
        </QueryClientProvider>
      </ThemeProvider>
    </ErrorBoundary>
  );
}

export default App;
