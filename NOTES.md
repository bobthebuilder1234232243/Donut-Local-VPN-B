# Notes for maintainers

Not user-facing. These are the things to keep in mind about the 0.2.0 change.

## 1. The TUN work is real engineering

The macOS/Linux tunnel is roughly 1,500 lines of new Rust in `src-tauri/src/engine/tunnel/`
(userspace TCP stack, DNS responder, route management, privileged helper and its control
protocol). It is not part of the rebrand and should not be reviewed as copy changes. It needs
testing on each supported distribution and macOS version: TUN device creation, route and DNS
restoration after a crash, and the helper hand-off between profile search and activation.

## 2. "Local VPN" is a naming choice, not a privacy claim

Traffic goes through a virtual interface on the same machine. There is no remote server, no
added encryption and no IP change. The README and the About screen say so explicitly; keep it
that way so the name cannot be read as a promise of anonymity.

## 3. The Linux permission model changed

FreeDPI advertised "Linux without root". Donut Local VPN needs root or `CAP_NET_ADMIN` to create
`/dev/net/tun` and change the default route. This must be called out in the release notes; users
who relied on the old claim will otherwise think the app broke.

## 4. TUN could have shipped separately

The rebrand and the tunnel are independent. If the tunnel had been too large for one pass, the
right split would have been: ship the rebrand first (strings, identifier, icons, updater key),
then add the TUN interface as its own PR. Keep that in mind for future large changes.

## Implementation notes

- **Updater key.** 0.2.0 is signed with a new Tauri keypair. The public key is in
  `src-tauri/tauri.conf.json`; the private key must be stored as the GitHub Actions secrets
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The password is empty
  for the key generated for this release, so the secret is the key file's contents and the
  password secret is an empty string. FreeDPI installs cannot auto-update: the key does not
  match and the endpoint points at this fork.
- **Release assets.** `.github/scripts/release-assets.mjs` renames the bundles to the agreed
  names (`Donut-Local-VPN_<version>_amd64.AppImage`, `..._amd64.deb`, `..._universal.dmg`,
  `..._x64-setup.exe`, `Donut-Local-VPN_universal.app.tar.gz`) and builds `latest.json` with the
  signatures from the build matrix. The workflow fails early if the signing key is missing.
- **Tunnel scope.** The macOS/Linux tunnel handles TCP and DNS; other UDP and ICMP take the
  normal route. This is documented in the README rather than hidden, because QUIC and some games
  are affected.
- **Settings migration.** On first start the app copies settings from the old
  `com.freedpi.desktop` configuration directory and leaves the old directory untouched.
- **Icons.** `src-tauri/icons/` and `docs/screenshots/` were regenerated for this release; the
  donut mark is drawn procedurally (`.icon-build.py` was a one-off and is not committed).
- **What was verified without a GUI toolchain.** The tunnel, DNS and route code has unit tests
  (`cd src-tauri && cargo test`, 27 passing). The frontend type-checks and builds with
  `npm run build`. A full `cargo check` of the Tauri binary needs GTK/WebKit development headers
  that are not installed in the build container, so the final desktop build is verified by CI.
