#!/usr/bin/env node
/**
 * Turns Tauri's bundle output into the names Donut Local VPN publishes and builds the
 * Tauri updater manifest.
 *
 *   node release-assets.mjs collect <windows|macos|linux> <bundle dir> [...more dirs]
 *   node release-assets.mjs merge <directory with downloaded artifacts>
 */
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join } from "node:path";

const REPO = "bobthebuilder1234232243/Donut-Local-VPN-B";
const BASE = `https://github.com/${REPO}/releases/latest/download`;
const OUT = "release-assets";
const version = JSON.parse(readFileSync("package.json", "utf8")).version;
const notes = readFileSync(".github/release-notes.md", "utf8").replaceAll("{{version}}", version);

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
  // Tauri may emit an AppImage tarball as well; the updater uses the AppImage itself.
  if (/\.AppImage\.tar\.gz$/i.test(name)) return null;
  if (/\.app\.tar\.gz$/i.test(name)) {
    return { platform: "macos", canonical: "Donut-Local-VPN_universal.app.tar.gz", signed: true };
  }
  if (/\.dmg$/i.test(name)) {
    return { platform: "macos", canonical: `Donut-Local-VPN_${version}_universal.dmg`, signed: false };
  }
  if (/\.AppImage$/i.test(name)) {
    return { platform: "linux", canonical: `Donut-Local-VPN_${version}_amd64.AppImage`, signed: true };
  }
  if (/\.deb$/i.test(name)) {
    return { platform: "linux", canonical: `Donut-Local-VPN_${version}_amd64.deb`, signed: false };
  }
  if (/setup\.exe$/i.test(name)) {
    return { platform: "windows", canonical: `Donut-Local-VPN_${version}_x64-setup.exe`, signed: true };
  }
  return null;
}

function collect(platform, roots) {
  mkdirSync(OUT, { recursive: true });
  const platforms = {};
  const shipped = [];

  for (const file of roots.flatMap(walk)) {
    const info = describe(file);
    if (!info || info.platform !== platform) continue;

    const target = join(OUT, info.canonical);
    copyFileSync(file, target);
    shipped.push(info.canonical);

    if (!info.signed) continue;

    const signatureFile = `${file}.sig`;
    if (!existsSync(signatureFile)) {
      throw new Error(
        `No signature next to ${basename(file)}. The build must run with TAURI_SIGNING_PRIVATE_KEY set.`,
      );
    }
    const signature = readFileSync(signatureFile, "utf8").trim();
    if (!signature) throw new Error(`Empty signature for ${basename(file)}`);

    copyFileSync(signatureFile, `${target}.sig`);
    shipped.push(`${info.canonical}.sig`);

    const entry = { signature, url: `${BASE}/${info.canonical}` };
    if (platform === "macos") {
      // A universal build serves both architectures.
      platforms["darwin-aarch64"] = entry;
      platforms["darwin-x86_64"] = entry;
    } else if (platform === "windows") {
      platforms["windows-x86_64"] = entry;
    } else {
      platforms["linux-x86_64"] = entry;
    }
  }

  if (shipped.length === 0) {
    throw new Error(`No ${platform} bundles found in ${roots.join(", ")}`);
  }

  writeFileSync(
    join(OUT, `latest-${platform}.json`),
    `${JSON.stringify({ version, notes, pub_date: new Date().toISOString(), platforms }, null, 2)}\n`,
  );

  console.log(`${platform}: ${shipped.join(", ")}`);
  if (Object.keys(platforms).length === 0) {
    console.log(`${platform}: no updater entry (installer only)`);
  }
}

function merge(dir) {
  const platforms = {};
  for (const name of readdirSync(dir)) {
    if (!/^latest-.+\.json$/.test(name)) continue;
    const manifest = JSON.parse(readFileSync(join(dir, name), "utf8"));
    Object.assign(platforms, manifest.platforms);
  }

  const required = ["windows-x86_64", "darwin-aarch64", "darwin-x86_64", "linux-x86_64"];
  const missing = required.filter((key) => !platforms[key]);
  if (missing.length > 0) {
    throw new Error(`latest.json is missing platforms: ${missing.join(", ")}`);
  }

  writeFileSync(
    join(dir, "latest.json"),
    `${JSON.stringify({ version, notes, pub_date: new Date().toISOString(), platforms }, null, 2)}\n`,
  );
  console.log(`latest.json: ${Object.keys(platforms).join(", ")}`);
}

const [mode, ...args] = process.argv.slice(2);
if (mode === "collect" && args.length >= 2) {
  collect(args[0], args.slice(1));
} else if (mode === "merge" && args.length === 1) {
  merge(args[0]);
} else {
  console.error("usage: release-assets.mjs collect <platform> <dir...> | merge <dir>");
  process.exit(1);
}
