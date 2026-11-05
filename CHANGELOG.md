# Changelog

## Unreleased

- New session details panel on the main screen while connected: tunnel IP, device IP, DPI bypass
  state, session uptime and bytes sent and received, polled from the engine once a second.
- New DPI bypass switch. On keeps TLS splitting and DNS over HTTPS; off forwards traffic
  unchanged and uses the system DNS. The engine checks it on every connection and the Windows
  driver always runs the bypass.
- New Connect on launch setting, separate from Start with system.
- The existing self-healing setting is now called Reconnect after network change and is
  documented next to the other connection settings.
- New editable Tunnel IP and Device IP settings (validated IPv4, optional prefix) applied on the
  next connection. Still no remote server: the tunnel is local.
- New connection log panel with info/warning/error filtering and a copy button. The log keeps
  structured levels in memory and in the log file.
- Added Spanish, Italian, Polish, Korean, Traditional Chinese and French. The app detects the
  system locale and falls back to English; all UI strings, including the new settings, are
  translated.
- New CI job builds the engine and runs a real TUN integration test in a privileged container:
  interface and routes come up, TCP is relayed without looping, DNS works over DoH, and the
  teardown restores the network. It runs with the bypass on and off, and the release workflow now
  depends on it.

## 0.2.0

Donut Local VPN is the new name of this fork. It is not an update of FreeDPI: the signing key
changed, so FreeDPI installs cannot auto-update and have to install this version by hand.

- Renamed to Donut Local VPN everywhere: window, tray, notifications, installer and About screen.
  New identifier `com.donutvpn.donutlocalvpn`, new icons and a new download page.
- macOS and Linux now route all traffic through a local TUN interface instead of a system proxy.
  Every app is covered; there is no proxy address to copy and no per-app configuration.
- DNS inside the tunnel is answered with DNS over HTTPS on macOS and Linux.
- Linux now requires root or `CAP_NET_ADMIN`; the app asks through `pkexec` or `sudo`.
- macOS asks for the administrator password when you connect. The helper exits and restores the
  original routes when you disconnect.
- Removed the "Open Discord" button, the manual proxy address and the system-proxy instructions.
- New update signing key and endpoint. Builds are signed in GitHub Actions; `latest.json` is
  generated from the build matrix.
- Windows is unchanged: same WinDivert driver and GoodbyeDPI engine.
- Settings from the old `com.freedpi.desktop` folder are imported on first start.

## 0.1.19

- Updates show up without restarting FreeDPI: it checks every 30 minutes and every time you open the window.
- New After VPN setting: when a VPN or a network change breaks the connection, FreeDPI reconnects on its own once the VPN is turned off or the network changes again. On by default.

## 0.1.18

- Linux support with .deb and AppImage packages.
- On Linux, FreeDPI sets the GNOME and KDE system proxy and shows the proxy address for other apps.

## 0.1.17

- Notifications when FreeDPI connects, disconnects or cannot connect.
- Turn notifications on or off under Settings > Notifications.

## 0.1.16

- Shows how long you have been connected.
- Notifies you when the connection stops working and when FreeDPI reconnects.

## 0.1.15

- Release notes now appear in the update banner.

## 0.1.14

- Choose between system, light and dark theme in Settings.

## 0.1.13

- Reconnects right after the computer wakes from sleep or the network changes.
- Connection checks are skipped while offline, so a lost Wi-Fi no longer triggers a needless search.
- New Help section under Settings > Advanced: copy diagnostics and report a problem.
- FreeDPI now keeps a log file to make bug reports easier to investigate.
- Release notes are shown in the update banner.
- Dark theme that follows the system setting.
- Screen reader improvements.

## 0.1.12

- Ice blue power button when disconnected.

## 0.1.11

- White theme and accordion settings with descriptions.

## 0.1.10

- Only test sites mode.

## 0.1.9

- Stops the WinDivert driver before installing and updating.
