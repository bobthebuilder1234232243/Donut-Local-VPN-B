import pkg from "../../package.json";
import type { Backend, Phase, Settings, Status } from "../types";

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
        platform: params.get("platform") ?? "windows",
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
