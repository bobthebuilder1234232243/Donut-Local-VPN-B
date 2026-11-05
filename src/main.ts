import { createBackend } from "./api";
import { format, languages, resolveLanguage, strings } from "./i18n";
import type { Backend, Language, Preferences, Region, Settings, Snapshot, Status, UpdateInfo } from "./types";

const REGIONS: Region[] = ["auto", "turkey", "russia", "other"];
const THEMES = ["system", "light", "dark"] as const;
type Theme = (typeof THEMES)[number];
const THEME_KEY = "donut-local-vpn-theme";
const CHECK_ICON = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>';
const REMOVE_ICON = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>';
const DOMAIN = /^(?!-)[a-z0-9-]{1,63}(?<!-)(\.(?!-)[a-z0-9-]{1,63}(?<!-))+$/;

const byId = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const ui = {
  app: byId("app"),
  power: byId<HTMLButtonElement>("btn-power"),
  title: byId("status-title"),
  detail: byId("status-detail"),
  profile: byId("status-profile"),
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
  version: byId("version"),
  quit: byId<HTMLButtonElement>("btn-quit"),
  update: byId("update"),
  updateText: byId("update-text"),
  updateButton: byId<HTMLButtonElement>("btn-update"),
  updateNotes: byId("update-notes"),
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
let update: UpdateInfo | null = null;
let updating = false;
let flashMessage = "";
let flashTimer: number | undefined;

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
  ui.vpnTitle.textContent = s.afterVpn;
  ui.vpnValue.textContent = settings.afterVpn ? s.on : s.off;
  ui.vpnLabel.textContent = s.afterVpnToggle;
  ui.vpnHint.textContent = s.afterVpnHint;
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
  ui.version.textContent = `Donut Local VPN ${snapshot.version}`;
  ui.source.title = s.source;
  ui.quit.textContent = s.quit;
  ui.helpTitle.textContent = s.help;
  ui.helpHint.textContent = s.helpHint;
  if (!ui.diagnostics.classList.contains("done")) ui.diagnostics.textContent = s.copyDiagnostics;
  ui.report.textContent = s.reportIssue;
  ui.aboutTitle.textContent = s.about;
  ui.aboutText.textContent = s.aboutText;
}

function renderUpdate() {
  const s = text();
  ui.update.hidden = update === null;
  if (!update) return;
  ui.updateText.textContent = updating ? s.updating : format(s.update, { version: update.version });
  ui.updateButton.textContent = s.updateAction;
  ui.updateButton.hidden = updating;
  const notes = update.notes?.trim() ?? "";
  ui.updateNotes.textContent = notes;
  ui.updateNotes.hidden = notes === "" || updating;
}

function render() {
  renderSettings();
  renderStatus();
  renderUpdate();
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

async function installUpdate() {
  updating = true;
  renderUpdate();
  try {
    await backend.installUpdate();
  } catch {
    updating = false;
    update = null;
    renderUpdate();
    flash(text().updateError);
  }
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
  ui.updateButton.addEventListener("click", () => void installUpdate());

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
  update = snapshot.update;
  document.documentElement.dataset.platform = snapshot.platform;

  backend.onStatus((next) => {
    status = next;
    renderStatus();
  });
  backend.onSettings((next) => {
    settings = next;
    renderSettings();
    renderStatus();
    renderUpdate();
  });
  backend.onUpdate((next) => {
    update = next;
    renderUpdate();
  });

  bind();
  window.setInterval(() => {
    if (status.phase === "on") renderProfile();
  }, 1000);
  const devParams = new URLSearchParams(location.search);
  openSheet(import.meta.env.DEV && devParams.has("settings"));
  if (import.meta.env.DEV && devParams.has("shot")) document.body.style.width = "380px";
  const devOpen = import.meta.env.DEV ? devParams.get("open") : null;
  if (devOpen) (document.getElementById(devOpen) as HTMLDetailsElement | null)?.setAttribute("open", "");
  render();
  document.body.classList.add("ready");
}

void boot();
