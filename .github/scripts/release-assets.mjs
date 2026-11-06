#!/usr/bin/env node
/**
 * Turns Tauri's bundle output into the names Donut Local VPN publishes.
 *
 *   node release-assets.mjs collect <windows|macos|linux> <bundle dir> [...more dirs]
 */
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
} from "node:fs";
import { basename, join } from "node:path";

const OUT = "release-assets";
const version = JSON.parse(readFileSync("package.json", "utf8")).version;

function walk(dir) {
  if (!existsSync(dir)) return [];
  const found = [];
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) found.push(...walk(path));
    else found.push(path);
  }
  return found;
}

/** Canonical published name for a bundle, or null when the file is not shipped. */
function describe(file) {
  const name = basename(file);
  if (name.endsWith(".sig")) return null;
  if (/\.dmg$/i.test(name)) {
    return { platform: "macos", canonical: `Donut-Local-VPN_${version}_universal.dmg` };
  }
  if (/\.AppImage$/i.test(name)) {
    return { platform: "linux", canonical: `Donut-Local-VPN_${version}_amd64.AppImage` };
  }
  if (/\.deb$/i.test(name)) {
    return { platform: "linux", canonical: `Donut-Local-VPN_${version}_amd64.deb` };
  }
  if (/setup\.exe$/i.test(name)) {
    return { platform: "windows", canonical: `Donut-Local-VPN_${version}_x64-setup.exe` };
  }
  return null;
}

function collect(platform, roots) {
  mkdirSync(OUT, { recursive: true });
  const shipped = [];

  for (const file of roots.flatMap(walk)) {
    const info = describe(file);
    if (!info || info.platform !== platform) continue;
    copyFileSync(file, join(OUT, info.canonical));
    shipped.push(info.canonical);
  }

  if (shipped.length === 0) {
    throw new Error(`No ${platform} bundles found in ${roots.join(", ")}`);
  }

  console.log(`${platform}: ${shipped.join(", ")}`);
}

const [mode, ...args] = process.argv.slice(2);
if (mode === "collect" && args.length >= 2) {
  collect(args[0], args.slice(1));
} else {
  console.error("usage: release-assets.mjs collect <platform> <dir...>");
  process.exit(1);
}
