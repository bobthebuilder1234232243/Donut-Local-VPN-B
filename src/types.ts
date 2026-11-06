export type Phase = "off" | "connecting" | "on" | "stopping" | "error";

export type Region = "auto" | "turkey" | "russia" | "other";

export type Language = "en" | "tr" | "ru" | "es" | "it" | "pl" | "ko" | "zh-TW" | "fr";

export type LogLevel = "info" | "warn" | "error";

export interface Status {
  phase: Phase;
  step: number;
  total: number;
  profile: string | null;
  error: string | null;
  since?: number | null;
  port?: number | null;
}

export interface SessionStats {
  tunnelIp: string | null;
  deviceIp: string | null;
  dpi: boolean;
  uptimeSecs: number;
  bytesUp: number | null;
  bytesDown: number | null;
}

export interface LogEntry {
  time: string;
  level: LogLevel;
  message: string;
}

export interface Settings {
  region: Region;
  targets: string[];
  custom: string[];
  selective: boolean;
  autostart: boolean;
  language: string;
  profile: string | null;
  engineActive: boolean;
  notifications: boolean;
  afterVpn: boolean;
  autoConnect: boolean;
  dpi: boolean;
  tunnelIp: string;
  deviceIp: string;
}

export interface Preferences {
  region: Region;
  targets: string[];
  custom: string[];
  selective: boolean;
}

export interface TargetInfo {
  id: string;
  name: string;
}

export interface Snapshot {
  status: Status;
  settings: Settings;
  targets: TargetInfo[];
  platform: string;
  version: string;
}

export interface Backend {
  init(language: string): Promise<Snapshot>;
  connect(): Promise<void>;
  disconnect(): Promise<void>;
  savePreferences(preferences: Preferences): Promise<Settings>;
  setLanguage(language: Language): Promise<Settings>;
  setAutostart(enabled: boolean): Promise<Settings>;
  setNotifications(enabled: boolean): Promise<Settings>;
  setAfterVpn(enabled: boolean): Promise<Settings>;
  setAutoConnect(enabled: boolean): Promise<Settings>;
  setDpi(enabled: boolean): Promise<Settings>;
  setNetwork(tunnelIp: string, deviceIp: string): Promise<Settings>;
  sessionStats(): Promise<SessionStats | null>;
  logs(): Promise<LogEntry[]>;
  quit(): Promise<void>;
  hide(): Promise<void>;
  onStatus(handler: (status: Status) => void): void;
  onSettings(handler: (settings: Settings) => void): void;
  openRepository(): Promise<void>;
  reportIssue(): Promise<void>;
  diagnostics(): Promise<string>;
}
