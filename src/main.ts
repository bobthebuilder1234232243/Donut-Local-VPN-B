import { createBackend } from "./api";
import { format, languages, resolveLanguage, strings } from "./i18n";
import type { Backend, Language, LogEntry, Preferences, Region, SessionStats, Settings, Snapshot, Status } from "./types";

const REGIONS: Region[] = ["auto", "turkey", "russia", "other"];
const THEMES = ["system", "light", "dark"] as const;
type Theme = (typeof THEMES)[number];
const THEME_KEY = "donut-local-vpn-theme";
const CHECK_ICON = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>';
const REMOVE_ICON = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>';
const DOMAIN = /^(?!-)[a-z0-9-]{1,63}(?<!-)(\.(?!-)[a-z0-9-]{1,63}(?<!-))+$/;
const IPV4 = /^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}(\/(3[0-2]|[12]?\d))?$/;

const byId = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const ui = {
  app: byId("app"),
  power: byId<HTMLButtonElement>("btn-power"),
  title: byId("status-title"),
  detail: byId("status-detail"),
  profile: byId("status-profile"),
  session: byId("session"),
  sessionTunnelLabel: byId("lbl-session-tunnel"),
  sessionDeviceLabel: byId("lbl-session-device"),
  sessionDpiLabel: byId("lbl-session-dpi"),
  sessionUptimeLabel: byId("lbl-session-uptime"),
  sessionUploadLabel: byId("lbl-session-upload"),
  sessionDownloadLabel: byId("lbl-session-download"),
  sessionTunnel: byId("val-session-tunnel"),
  sessionDevice: byId("val-session-device"),
  sessionDpi: byId("val-session-dpi"),
  sessionUptime: byId("val-session-uptime"),
  sessionUpload: byId("val-session-upload"),
  sessionDownload: byId("val-session-download"),
  autostartLabel: byId("lbl-autostart"),
  autostart: byId<HTMLInputElement>("chk-autostart"),
  openSettings: byId<HTMLButtonElement>("btn-settings"),
  hide: byId<HTMLButtonElement>("btn-hide"),
  back: byId<HTMLButtonElement>("btn-back"),
  sheet: byId("sheet"),
  settingsTitle: byId("lbl-settings"),
  pending: byId("lbl-pending"),
  regionTitle: byId("lbl-region"),
  regionHint: byId("lbl-region-hint"),
  region: byId("region"),
  languageTitle: byId("lbl-language"),
  language: byId("language"),
  source: byId<HTMLButtonElement>("btn-source"),
  sitesTitle: byId("lbl-sites"),
  sitesHint: byId("lbl-sites-hint"),
  siteTiles: byId("site-tiles"),
  advancedTitle: byId("lbl-advanced"),
  valRegion: byId("val-region"),
  valLanguage: byId("val-language"),
  valAdvanced: byId("val-advanced"),
  languageHint: byId("lbl-language-hint"),
  selective: byId<HTMLInputElement>("chk-selective"),
  selectiveLabel: byId("lbl-selective"),
  selectiveHint: byId("lbl-selective-hint"),
  addForm: byId<HTMLFormElement>("add-form"),
  addInput: byId<HTMLInputElement>("add-input"),
  addButton: byId<HTMLButtonElement>("btn-add"),
  quit: byId<HTMLButtonElement>("btn-quit"),
  helpTitle: byId("lbl-help"),
  helpHint: byId("lbl-help-hint"),
  diagnostics: byId<HTMLButtonElement>("btn-diagnostics"),
  report: byId<HTMLButtonElement>("btn-report"),
  themeTitle: byId("lbl-theme"),
  themeHint: byId("lbl-theme-hint"),
  theme: byId("theme"),
  valTheme: byId("val-theme"),
  notificationsTitle: byId("lbl-notifications"),
  notificationsValue: byId("val-notifications"),
  notificationsLabel: byId("lbl-notify-toggle"),
  notificationsHint: byId("lbl-notifications-hint"),
  notifications: byId<HTMLInputElement>("chk-notifications"),
  vpnTitle: byId("lbl-vpn"),
  vpnValue: byId("val-vpn"),
  vpnLabel: byId("lbl-vpn-toggle"),
  vpnHint: byId("lbl-vpn-hint"),
  vpn: byId<HTMLInputElement>("chk-vpn"),
  aboutTitle: byId("lbl-about"),
  aboutText: byId("lbl-about-text"),
  dpiTitle: byId("lbl-dpi"),
  dpiValue: byId("val-dpi"),
  dpiLabel: byId("lbl-dpi-toggle"),
  dpiHint: byId("lbl-dpi-hint"),
  dpi: byId<HTMLInputElement>("chk-dpi"),
  networkTitle: byId("lbl-network"),
  networkValue: byId("val-network"),
  networkHint: byId("lbl-network-hint"),
  tunnelIpLabel: byId("lbl-tunnel-ip"),
  deviceIpLabel: byId("lbl-device-ip"),
  tunnelIpInput: byId<HTMLInputElement>("inp-tunnel-ip"),
  deviceIpInput: byId<HTMLInputElement>("inp-device-ip"),
  networkApply: byId<HTMLButtonElement>("btn-network-apply"),
  networkError: byId("lbl-network-error"),
  autoConnectTitle: byId("lbl-autoconnect"),
  autoConnectValue: byId("val-autoconnect"),
  autoConnectLabel: byId("lbl-autoconnect-toggle"),
  autoConnectHint: byId("lbl-autoconnect-hint"),
  autoConnect: byId<HTMLInputElement>("chk-autoconnect"),
  logsTitle: byId("lbl-logs"),
  logsHint: byId("lbl-logs-hint"),
  logLevels: byId("log-levels"),
  logList: byId("log-list"),
  logEmpty: byId("lbl-log-empty"),
  copyLogs: byId<HTMLButtonElement>("btn-copy-logs"),
  refreshLogs: byId<HTMLButtonElement>("btn-refresh-logs"),
};

function storedTheme(): Theme {
  try {
    const value = localStorage.getItem(THEME_KEY);
    return THEMES.includes(value as Theme) ? (value as Theme) : "system";
  } catch {
    return "system";
  }
}

function applyTheme(theme: Theme) {
  if (theme === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

let theme: Theme = storedTheme();
applyTheme(theme);

let backend: Backend;
let snapshot: Snapshot;
let status: Status;
let settings: Settings;
let flashMessage = "";
let flashTimer: number | undefined;
let stats: SessionStats | null = null;
let statsBusy = false;
let logEntries: LogEntry[] = [];
let logFilter: "all" | "info" | "warn" | "error" = "all";
let logsBusy = false;
let logsTimer: number | undefined;

const language = (): Language => resolveLanguage(settings.language || navigator.language);
const text = () => strings(language());

function detailText(): string {
  const s = text();
  if (flashMessage) return flashMessage;
  switch (status.phase) {
    case "off":
      return s.detail.off;
    case "connecting":
      return status.total > 0 ? format(s.detail.trying, { step: status.step, total: status.total }) : s.detail.preparing;
    case "on":
      return s.detail.on;
    case "stopping":
      return s.detail.stopping;
    case "error": {
      const message = s.errors[status.error ?? "engine"] ?? s.errors.engine;
      return status.error === "no_profile" && snapshot.platform === "windows" ? `${message} ${s.kaspersky}` : message;
    }
  }
}

function elapsed(since: number): string {
  const total = Math.max(0, Math.floor((Date.now() - since) / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const sec = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}

function renderProfile() {
  const s = text();
  if (status.phase !== "on") {
    ui.profile.textContent = "";
    return;
  }
  const parts: string[] = [];
  if (status.since) parts.push(format(s.connectedFor, { time: elapsed(status.since) }));
  if (status.step > 0) parts.push(format(s.profile, { step: status.step }));
  ui.profile.textContent = parts.join(" · ");
}

function formatUptime(total: number): string {
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const sec = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(h)}:${pad(m)}:${pad(sec)}`;
}

function formatBytes(value: number | null): string {
  if (value === null) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${Math.round(size)} ${units[unit]}` : `${size.toFixed(1)} ${units[unit]}`;
}

function renderSession() {
  const s = text();
  ui.session.hidden = status.phase !== "on";
  if (ui.session.hidden) return;
  ui.sessionTunnelLabel.textContent = s.session.tunnelIp;
  ui.sessionDeviceLabel.textContent = s.session.deviceIp;
  ui.sessionDpiLabel.textContent = s.session.dpi;
  ui.sessionUptimeLabel.textContent = s.session.uptime;
  ui.sessionUploadLabel.textContent = s.session.upload;
  ui.sessionDownloadLabel.textContent = s.session.download;
  ui.sessionTunnel.textContent = stats?.tunnelIp ?? "—";
  ui.sessionDevice.textContent = stats?.deviceIp ?? "—";
  ui.sessionDpi.textContent = stats ? (stats.dpi ? s.on : s.off) : "—";
  ui.sessionUptime.textContent = stats ? formatUptime(stats.uptimeSecs) : "—";
  ui.sessionUpload.textContent = formatBytes(stats?.bytesUp ?? null);
  ui.sessionDownload.textContent = formatBytes(stats?.bytesDown ?? null);
}

async function refreshStats() {
  if (statsBusy || status.phase !== "on") return;
  statsBusy = true;
  try {
    stats = await backend.sessionStats();
  } catch {
    stats = null;
  }
  statsBusy = false;
  renderSession();
}

function renderStatus() {
  const s = text();
  if (ui.app.dataset.phase !== status.phase) {
    for (const element of [ui.title, ui.detail]) {
      element.classList.remove("rise");
      void element.offsetWidth;
      element.classList.add("rise");
    }
  }
  ui.app.dataset.phase = status.phase;
  ui.title.textContent = s.phase[status.phase];
  ui.detail.textContent = detailText();
  renderProfile();

  const action =
    status.phase === "on" ? s.action.disconnect : status.phase === "connecting" ? s.action.cancel : s.action.connect;
  ui.power.title = action;
  ui.power.setAttribute("aria-label", action);
  ui.power.disabled = status.phase === "stopping";
  ui.pending.hidden = !(status.phase === "on" || status.phase === "connecting");

  ui.power.setAttribute("aria-busy", String(status.phase === "connecting" || status.phase === "stopping"));
  if (status.phase !== "on") stats = null;
  renderSession();
}

function option(label: string, selected: boolean, mark: "radio" | "check" | "remove", onClick: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = `opt ${mark}`;
  button.classList.toggle("selected", selected);
  button.setAttribute("aria-pressed", String(selected));
  const indicator = document.createElement("span");
  indicator.className = "indicator";
  if (mark === "check") indicator.innerHTML = CHECK_ICON;
  if (mark === "remove") indicator.innerHTML = REMOVE_ICON;
  const text = document.createElement("span");
  text.className = "opt-label";
  text.textContent = label;
  if (mark === "remove") button.append(text, indicator);
  else button.append(indicator, text);
  button.addEventListener("click", onClick);
  return button;
}

function choices<T extends string>(
  container: HTMLElement,
  items: { id: T; label: string }[],
  active: T,
  select: (id: T) => void,
) {
  container.replaceChildren(
    ...items.map((item) =>
      option(item.label, item.id === active, "radio", () => {
        closeAccordions();
        if (item.id !== active) select(item.id);
      }),
    ),
  );
}

function closeAccordions() {
  document.querySelectorAll<HTMLDetailsElement>("details.acc").forEach((d) => (d.open = false));
}

function renderSites() {
  const s = text();
  const presets = snapshot.targets.map((target) => {
    const active = settings.targets.includes(target.id);
    return option(target.name, active, "check", () => {
      const targets = active ? settings.targets.filter((id) => id !== target.id) : [...settings.targets, target.id];
      void save({ targets });
    });
  });
  const custom = settings.custom.map((domain) => {
    const button = option(domain, true, "remove", () => void save({ custom: settings.custom.filter((d) => d !== domain) }));
    button.title = `${s.remove} ${domain}`;
    return button;
  });
  ui.siteTiles.replaceChildren(...presets, ...custom);
}

function renderSettings() {
  const s = text();
  document.documentElement.lang = language();
  ui.autostartLabel.textContent = s.autostart;
  ui.autostart.checked = settings.autostart;
  ui.openSettings.title = s.settings;
  ui.openSettings.setAttribute("aria-label", s.settings);
  ui.hide.title = s.hide;
  ui.hide.setAttribute("aria-label", s.hide);
  ui.back.title = s.back;
  ui.back.setAttribute("aria-label", s.back);
  ui.settingsTitle.textContent = s.settings;
  ui.pending.textContent = s.pending;
  ui.regionTitle.textContent = s.region;
  ui.regionHint.textContent = s.regionHint;
  ui.valRegion.textContent = s.regions[settings.region];
  choices(ui.region, REGIONS.map((id) => ({ id, label: s.regions[id] })), settings.region, (region) => void save({ region }));
  ui.dpiTitle.textContent = s.dpi;
  ui.dpiValue.textContent = settings.dpi ? s.on : s.off;
  ui.dpiLabel.textContent = s.dpiToggle;
  ui.dpiHint.textContent = snapshot.platform === "windows" ? s.dpiWindowsHint : s.dpiHint;
  ui.dpi.checked = snapshot.platform === "windows" ? true : settings.dpi;
  ui.dpi.disabled = snapshot.platform === "windows";
  ui.networkTitle.textContent = s.network;
  ui.networkValue.textContent = `${settings.tunnelIp} · ${settings.deviceIp}`;
  ui.networkHint.textContent = s.networkHint;
  ui.tunnelIpLabel.textContent = s.tunnelIp;
  ui.deviceIpLabel.textContent = s.deviceIp;
  if (document.activeElement !== ui.tunnelIpInput) ui.tunnelIpInput.value = settings.tunnelIp;
  if (document.activeElement !== ui.deviceIpInput) ui.deviceIpInput.value = settings.deviceIp;
  ui.networkApply.textContent = s.applyNetwork;
  ui.autoConnectTitle.textContent = s.autoConnect;
  ui.autoConnectValue.textContent = settings.autoConnect ? s.on : s.off;
  ui.autoConnectLabel.textContent = s.autoConnectToggle;
  ui.autoConnectHint.textContent = s.autoConnectHint;
  ui.autoConnect.checked = settings.autoConnect;
  ui.logsTitle.textContent = s.logs;
  ui.logsHint.textContent = s.logsHint;
  ui.copyLogs.textContent = s.copyLogs;
  ui.refreshLogs.textContent = s.refreshLogs;
  ui.logEmpty.textContent = s.logsEmpty;
  renderLogs();
  ui.vpnTitle.textContent = s.reconnect;
  ui.vpnValue.textContent = settings.afterVpn ? s.on : s.off;
  ui.vpnLabel.textContent = s.reconnectToggle;
  ui.vpnHint.textContent = s.reconnectHint;
  ui.vpn.checked = settings.afterVpn;
  ui.notificationsTitle.textContent = s.notifications;
  ui.notificationsValue.textContent = settings.notifications ? s.on : s.off;
  ui.notificationsLabel.textContent = s.notificationsToggle;
  ui.notificationsHint.textContent = s.notificationsHint;
  ui.notifications.checked = settings.notifications;
  ui.themeTitle.textContent = s.theme;
  ui.themeHint.textContent = s.themeHint;
  ui.valTheme.textContent = s.themes[theme];
  choices(ui.theme, THEMES.map((id) => ({ id, label: s.themes[id] })), theme, (next) => {
    theme = next;
    try {
      localStorage.setItem(THEME_KEY, next);
    } catch {}
    applyTheme(next);
    renderSettings();
  });
  ui.languageTitle.textContent = s.language;
  ui.languageHint.textContent = s.languageHint;
  ui.valLanguage.textContent = languages.find((l) => l.id === language())?.label ?? "";
  choices(ui.language, languages, language(), (next) => void changeLanguage(next));
  const selectedSites = [
    ...snapshot.targets.filter((t) => settings.targets.includes(t.id)).map((t) => t.name),
    ...settings.custom,
  ];
  ui.valAdvanced.textContent = selectedSites.length > 2 ? `${selectedSites.slice(0, 2).join(", ")} +${selectedSites.length - 2}` : selectedSites.join(", ");
  ui.advancedTitle.textContent = s.advanced;
  ui.selectiveLabel.textContent = s.selective;
  ui.selectiveHint.textContent = s.selectiveHint;
  ui.selective.checked = settings.selective;
  ui.sitesTitle.textContent = s.sites;
  ui.sitesHint.textContent = s.sitesHint;
  renderSites();
  ui.addInput.placeholder = s.addPlaceholder;
  ui.addButton.title = s.addSite;
  ui.addButton.setAttribute("aria-label", s.addSite);
  ui.source.title = s.source;
  ui.source.setAttribute("aria-label", s.source);
  ui.quit.textContent = s.quit;
  ui.helpTitle.textContent = s.help;
  ui.helpHint.textContent = s.helpHint;
  if (!ui.diagnostics.classList.contains("done")) ui.diagnostics.textContent = s.copyDiagnostics;
  ui.report.textContent = s.reportIssue;
  ui.aboutTitle.textContent = s.about;
  ui.aboutText.textContent = s.aboutText;
}

function renderLogs() {
  const s = text();
  const levels: { id: typeof logFilter; label: string }[] = [
    { id: "all", label: s.logLevels.all },
    { id: "info", label: s.logLevels.info },
    { id: "warn", label: s.logLevels.warn },
    { id: "error", label: s.logLevels.error },
  ];
  ui.logLevels.replaceChildren(
    ...levels.map(({ id, label }) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = id === logFilter ? "level selected" : "level";
      button.textContent = label;
      button.setAttribute("aria-pressed", String(id === logFilter));
      button.addEventListener("click", () => {
        logFilter = id;
        renderLogs();
      });
      return button;
    }),
  );
  const visible = logEntries.filter((entry) => logFilter === "all" || entry.level === logFilter);
  ui.logList.replaceChildren(
    ...visible.map((entry) => {
      const line = document.createElement("div");
      line.className = "log-line";
      const time = document.createElement("span");
      time.className = "log-time";
      time.textContent = entry.time.length >= 19 ? entry.time.slice(11, 19) : entry.time;
      const level = document.createElement("span");
      level.className = `log-level ${entry.level}`;
      level.textContent = entry.level.toUpperCase();
      const message = document.createElement("span");
      message.className = "log-msg";
      message.textContent = entry.message;
      line.append(time, level, message);
      return line;
    }),
  );
  ui.logEmpty.hidden = visible.length > 0;
  ui.logList.scrollTop = ui.logList.scrollHeight;
}

async function refreshLogs() {
  if (logsBusy) return;
  logsBusy = true;
  try {
    logEntries = await backend.logs();
    renderLogs();
  } catch {
  } finally {
    logsBusy = false;
  }
}

async function copyLogs() {
  const visible = logEntries.filter((entry) => logFilter === "all" || entry.level === logFilter);
  const content = visible.map((entry) => `${entry.time} ${entry.level.toUpperCase()} ${entry.message}`).join("\n");
  try {
    await navigator.clipboard.writeText(content);
    ui.copyLogs.textContent = text().copied;
  } catch {
    ui.copyLogs.textContent = text().copyFailed;
  }
  window.setTimeout(() => {
    ui.copyLogs.textContent = text().copyLogs;
  }, 2000);
}

function watchLogs(active: boolean) {
  if (logsTimer !== undefined) {
    window.clearInterval(logsTimer);
    logsTimer = undefined;
  }
  if (!active) return;
  void refreshLogs();
  logsTimer = window.setInterval(() => void refreshLogs(), 2000);
}

function render() {
  renderSettings();
  renderStatus();
}

function flash(message: string) {
  flashMessage = message;
  window.clearTimeout(flashTimer);
  flashTimer = window.setTimeout(() => {
    flashMessage = "";
    renderStatus();
  }, 4000);
  renderStatus();
}

async function save(change: Partial<Preferences>) {
  const preferences: Preferences = {
    region: settings.region,
    targets: settings.targets,
    custom: settings.custom,
    selective: settings.selective,
    ...change,
  };
  settings = await backend.savePreferences(preferences);
  render();
}

async function changeLanguage(next: Language) {
  settings = await backend.setLanguage(next);
  render();
}

function normalizeDomain(input: string): string | null {
  const host = input
    .trim()
    .toLowerCase()
    .replace(/^https?:\/\//, "")
    .split(/[/?#:]/)[0]
    .replace(/\.$/, "");
  return DOMAIN.test(host) && host.length <= 253 ? host : null;
}

function openSheet(open: boolean) {
  ui.sheet.classList.toggle("open", open);
  ui.sheet.setAttribute("aria-hidden", String(!open));
  ui.sheet.inert = !open;
  if (!open) ui.addInput.classList.remove("invalid");
}

function bind() {
  ui.power.addEventListener("click", () => {
    if (status.phase === "off" || status.phase === "error") void backend.connect();
    else if (status.phase === "connecting" || status.phase === "on") void backend.disconnect();
  });

  ui.autostart.addEventListener("change", async () => {
    const wanted = ui.autostart.checked;
    try {
      settings = await backend.setAutostart(wanted);
    } catch {
      flash(text().autostartError);
    }
    render();
  });

  ui.source.addEventListener("click", () => void backend.openRepository());
  ui.report.addEventListener("click", () => void backend.reportIssue());
  ui.diagnostics.addEventListener("click", async () => {
    const s = text();
    try {
      await navigator.clipboard.writeText(await backend.diagnostics());
      ui.diagnostics.textContent = s.copied;
      ui.diagnostics.classList.add("done");
    } catch {
      ui.diagnostics.textContent = s.copyFailed;
    }
    window.setTimeout(() => {
      ui.diagnostics.classList.remove("done");
      ui.diagnostics.textContent = text().copyDiagnostics;
    }, 2500);
  });
  ui.selective.addEventListener("change", () => void save({ selective: ui.selective.checked }));
  ui.vpn.addEventListener("change", async () => {
    settings = await backend.setAfterVpn(ui.vpn.checked);
    render();
  });
  ui.notifications.addEventListener("change", async () => {
    settings = await backend.setNotifications(ui.notifications.checked);
    render();
  });
  ui.dpi.addEventListener("change", async () => {
    settings = await backend.setDpi(ui.dpi.checked);
    render();
  });
  ui.autoConnect.addEventListener("change", async () => {
    settings = await backend.setAutoConnect(ui.autoConnect.checked);
    render();
  });
  ui.networkApply.addEventListener("click", async () => {
    const tunnel = ui.tunnelIpInput.value.trim();
    const device = ui.deviceIpInput.value.trim();
    if (!IPV4.test(tunnel) || !IPV4.test(device)) {
      ui.networkError.textContent = text().invalidNetwork;
      return;
    }
    try {
      settings = await backend.setNetwork(tunnel, device);
      ui.networkError.textContent = "";
      flash(text().networkSaved);
      render();
    } catch {
      ui.networkError.textContent = text().invalidNetwork;
    }
  });
  ui.copyLogs.addEventListener("click", () => void copyLogs());
  ui.refreshLogs.addEventListener("click", () => void refreshLogs());
  const logsAccordion = document.getElementById("acc-logs") as HTMLDetailsElement | null;
  logsAccordion?.addEventListener("toggle", () => watchLogs(logsAccordion.open));

  ui.openSettings.addEventListener("click", () => openSheet(true));
  document.querySelectorAll<HTMLDetailsElement>("details.acc").forEach((details) =>
    details.addEventListener("toggle", () => {
      if (!details.open) return;
      document.querySelectorAll<HTMLDetailsElement>("details.acc").forEach((other) => {
        if (other !== details) other.open = false;
      });
    }),
  );
  ui.back.addEventListener("click", () => openSheet(false));
  ui.hide.addEventListener("click", () => void backend.hide());
  ui.quit.addEventListener("click", () => void backend.quit());

  ui.addForm.addEventListener("submit", (event) => {
    event.preventDefault();
    const domain = normalizeDomain(ui.addInput.value);
    if (!domain) {
      ui.addInput.classList.add("invalid");
      ui.addInput.title = text().invalidSite;
      return;
    }
    ui.addInput.classList.remove("invalid");
    ui.addInput.value = "";
    if (!settings.custom.includes(domain)) void save({ custom: [...settings.custom, domain] });
  });
  ui.addInput.addEventListener("input", () => ui.addInput.classList.remove("invalid"));

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") openSheet(false);
  });
  document.addEventListener("contextmenu", (event) => {
    if (!(event.target instanceof HTMLInputElement)) event.preventDefault();
  });
}

async function boot() {
  backend = await createBackend();
  snapshot = await backend.init(navigator.language);
  status = snapshot.status;
  settings = snapshot.settings;
  document.documentElement.dataset.platform = snapshot.platform;

  backend.onStatus((next) => {
    status = next;
    renderStatus();
  });
  backend.onSettings((next) => {
    settings = next;
    renderSettings();
    renderStatus();
  });

  bind();
  window.setInterval(() => {
    if (status.phase === "on") {
      renderProfile();
      void refreshStats();
    }
  }, 1000);
  const devParams = new URLSearchParams(location.search);
  openSheet(import.meta.env.DEV && devParams.has("settings"));
  if (import.meta.env.DEV && devParams.has("shot")) document.body.style.width = "380px";
  const devOpen = import.meta.env.DEV ? devParams.get("open") : null;
  if (devOpen) (document.getElementById(devOpen) as HTMLDetailsElement | null)?.setAttribute("open", "");
  render();
  if (status.phase === "on") void refreshStats();
  watchLogs(false);
  document.body.classList.add("ready");
}

void boot();
