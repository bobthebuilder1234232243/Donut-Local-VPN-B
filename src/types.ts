export type Phase = "off" | "connecting" | "on" | "stopping" | "error";

export type Region = "auto" | "turkey" | "russia" | "other";

export type Language = "en" | "tr" | "ru";

export interface Status {
  phase: Phase;
  step: number;
  total: number;
  profile: string | null;
  error: string | null;
  since?: number | null;
  port?: number | null;
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
  update: UpdateInfo | null;
}

export interface UpdateInfo {
  version: string;
  notes?: string | null;
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
  quit(): Promise<void>;
  hide(): Promise<void>;
  onStatus(handler: (status: Status) => void): void;
  onSettings(handler: (settings: Settings) => void): void;
  installUpdate(): Promise<void>;
  openDiscord(): Promise<void>;
  openRepository(): Promise<void>;
  reportIssue(): Promise<void>;
  diagnostics(): Promise<string>;
  onUpdate(handler: (update: UpdateInfo) => void): void;
}
