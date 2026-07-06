import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const keysDir = join(root, "apps/desktop/src-tauri/keys");
const pubPath = join(keysDir, "memorafy.key.pub");
const confPath = join(root, "apps/desktop/src-tauri/tauri.conf.json");

mkdirSync(keysDir, { recursive: true });

if (!existsSync(pubPath) && process.env.TAURI_SIGNING_PUBLIC_KEY) {
  writeFileSync(pubPath, `${process.env.TAURI_SIGNING_PUBLIC_KEY.trim()}\n`);
}

if (!existsSync(pubPath)) {
  console.error(`Missing ${pubPath}. Run scripts/generate-updater-keys or set TAURI_SIGNING_PUBLIC_KEY.`);
  process.exit(1);
}

const pubkey = readFileSync(pubPath, "utf8").trim();
const conf = JSON.parse(readFileSync(confPath, "utf8"));
conf.plugins ??= {};
conf.plugins.updater ??= {};
conf.plugins.updater.pubkey = pubkey;
writeFileSync(confPath, `${JSON.stringify(conf, null, 2)}\n`);
console.log("Synced updater pubkey into tauri.conf.json");
