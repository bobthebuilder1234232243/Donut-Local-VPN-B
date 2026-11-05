import pkg from "../../package.json";
import type { Backend, LogEntry, Phase, SessionStats, Settings, Status } from "../types";

const PROFILES = ["-5 --wrong-seq --reverse-frag", "-5 --set-ttl 5", "-5", "--set-ttl 3", "-9"];

function initialStatus(phase: string | null): Status {
  switch (phase as Phase | null) {
    case "connecting":
      return { phase: "connecting", step: 2, total: PROFILES.length, profile: null, error: null };
    case "on":
      return { phase: "on", step: 1, total: PROFILES.length, profile: PROFILES[0], error: null, since: Date.now() - 3725000, port: 18080 };
    case "error":
      return { phase: "error", step: 0, total: 0, profile: null, error: "no_profile" };
    default:
      return { phase: "off", step: 0, total: 0, profile: null, error: null };
  }
}

function sampleLogs(status: Status): LogEntry[] {
  const time = (offset: number) =>
    new Date(Date.now() - offset).toISOString().replace("T", " ").replace(/\.\d+Z$/, "Z");
  return [
    { time: time(180000), level: "info", message: "Donut Local VPN 0.2.0 started on linux" },
    { time: time(170000), level: "info", message: "search region=Auto targets=discord selective=false" },
    { time: time(165000), level: "info", message: "status Connecting" },
    { time: time(160000), level: "warn", message: "setting 1 did not work" },
    { time: time(155000), level: "info", message: "status On profile=split=1 disorder=1" },
    { time: time(90000), level: "warn", message: "health check failed (1/2)" },
    { time: time(60000), level: "info", message: "network changed" },
    { time: time(30000), level: "error", message: "status Error error=engine" },
    { time: time(1000), level: "info", message: `status ${status.phase}` },
  ];
}

export function mockBackend(): Backend {
  const params = new URLSearchParams(location.search);
  const fail = params.get("fail") !== null;
  let status = initialStatus(params.get("phase"));
  let settings: Settings = {
    region: "auto",
    targets: ["discord"],
    custom: [],
    selective: false,
    autostart: false,
    language: params.get("lang") ?? "",
    profile: null,
    engineActive: false,
    notifications: true,
    afterVpn: true,
    autoConnect: params.get("autoconnect") !== null,
    dpi: params.get("dpi") !== "0",
    tunnelIp: "10.66.0.1/24",
    deviceIp: "10.66.0.2",
  };
  const statusHandlers: ((status: Status) => void)[] = [];
  const settingsHandlers: ((settings: Settings) => void)[] = [];
  let timer: number | undefined;

  const emit = (next: Status) => {
    status = next;
    statusHandlers.forEach((handler) => handler(status));
  };
  const update = (next: Partial<Settings>) => {
    settings = { ...settings, ...next };
    settingsHandlers.forEach((handler) => handler(settings));
    return settings;
  };
  const stats = (): SessionStats | null => {
    if (status.phase !== "on") return null;
    const seconds = Math.max(0, Math.floor((Date.now() - (status.since ?? Date.now())) / 1000));
    return {
      tunnelIp: settings.dpi ? settings.tunnelIp : settings.tunnelIp,
      deviceIp: settings.deviceIp,
      dpi: settings.dpi,
      uptimeSecs: seconds,
      bytesUp: seconds * 24000,
      bytesDown: seconds * 96000,
    };
  };

  return {
    async init(language) {
      if (!settings.language) {
        settings.language = language.toLowerCase().startsWith("tr") ? "tr" : "en";
      }
      return {
        status,
        settings,
        targets: [
          { id: "discord", name: "Discord" },
          { id: "youtube", name: "YouTube" },
          { id: "roblox", name: "Roblox" },
          { id: "wattpad", name: "Wattpad" },
        ],
        platform: params.get("platform") ?? "linux",
        version: pkg.version,
        update: params.get("update") !== null ? { version: "0.2.0", notes: "- Faster connection after sleep\n- Diagnostics button" } : null,
      };
    },
    async connect() {
      if (status.phase === "connecting" || status.phase === "on") return;
      let step = 0;
      const tick = () => {
        step += 1;
        if (step > 3) {
          emit(
            fail
              ? { phase: "error", step: 0, total: 0, profile: null, error: "no_profile" }
              : { phase: "on", step: 3, total: PROFILES.length, profile: PROFILES[2], error: null, since: Date.now() },
          );
          return;
        }
        emit({ phase: "connecting", step, total: PROFILES.length, profile: null, error: null });
        timer = window.setTimeout(tick, 900);
      };
      tick();
    },
    async disconnect() {
      window.clearTimeout(timer);
      emit({ ...status, phase: "stopping" });
      timer = window.setTimeout(() => emit(initialStatus(null)), 500);
    },
    async savePreferences(preferences) {
      return update(preferences);
    },
    async setLanguage(language) {
      return update({ language });
    },
    async setAutostart(enabled) {
      return update({ autostart: enabled });
    },
    async setNotifications(enabled) {
      return update({ notifications: enabled });
    },
    async setAfterVpn(enabled) {
      return update({ afterVpn: enabled });
    },
    async setAutoConnect(enabled) {
      return update({ autoConnect: enabled });
    },
    async setDpi(enabled) {
      return update({ dpi: enabled });
    },
    async setNetwork(tunnelIp, deviceIp) {
      return update({ tunnelIp, deviceIp });
    },
    async sessionStats() {
      return stats();
    },
    async logs() {
      return sampleLogs(status);
    },
    async quit() {},
    async hide() {},
    onStatus(handler) {
      statusHandlers.push(handler);
    },
    onSettings(handler) {
      settingsHandlers.push(handler);
    },
    async installUpdate() {
      await new Promise((resolve) => window.setTimeout(resolve, 1500));
    },
    onUpdate() {},
    async reportIssue() {
      window.open("https://github.com/bobthebuilder1234232243/Donut-Local-VPN-B/issues/new", "_blank");
    },
    async diagnostics() {
      return `Donut Local VPN ${pkg.version}\nstatus: ${status.phase}`;
    },
    async openRepository() {
      window.open("https://github.com/bobthebuilder1234232243/Donut-Local-VPN-B", "_blank");
    },
  };
}
