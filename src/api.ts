import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import type { Backend, LogEntry, SessionStats, Settings, Status, UpdateInfo } from "./types";

function tauriBackend(): Backend {
  return {
    init: (language) => invoke("init", { language }),
    connect: () => invoke("connect"),
    disconnect: () => invoke("disconnect"),
    savePreferences: (preferences) => invoke("save_preferences", { preferences }),
    setLanguage: (language) => invoke("set_language", { language }),
    setAutostart: (enabled) => invoke("set_autostart", { enabled }),
    setNotifications: (enabled) => invoke("set_notifications", { enabled }),
    setAfterVpn: (enabled) => invoke("set_after_vpn", { enabled }),
    setAutoConnect: (enabled) => invoke("set_auto_connect", { enabled }),
    setDpi: (enabled) => invoke("set_dpi", { enabled }),
    setNetwork: (tunnelIp, deviceIp) => invoke("set_network", { tunnelIp, deviceIp }),
    sessionStats: () => invoke<SessionStats | null>("session_stats"),
    logs: () => invoke<LogEntry[]>("logs"),
    quit: () => invoke("quit"),
    hide: () => getCurrentWindow().hide(),
    onStatus: (handler) => {
      void listen<Status>("status", (event) => handler(event.payload));
    },
    onSettings: (handler) => {
      void listen<Settings>("settings", (event) => handler(event.payload));
    },
    installUpdate: () => invoke("install_update"),
    openRepository: () => invoke("open_repository"),
    reportIssue: () => invoke("report_issue"),
    diagnostics: () => invoke("diagnostics"),
    onUpdate: (handler) => {
      void listen<UpdateInfo>("update", (event) => handler(event.payload));
    },
  };
}

export async function createBackend(): Promise<Backend> {
  if ("__TAURI_INTERNALS__" in window) {
    return tauriBackend();
  }
  if (import.meta.env.DEV) {
    const { mockBackend } = await import("./dev/mock");
    return mockBackend();
  }
  throw new Error("Donut Local VPN must run inside the desktop app.");
}
