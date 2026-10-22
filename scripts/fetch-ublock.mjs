// Fetches the pinned uBlock Origin Lite release (Edge build, used by the
// Backloggd view's ad blocker), checks its SHA-256 and unpacks it into
// app/src-tauri/extensions/ubolite. Run once after cloning:
//   node scripts/fetch-ublock.mjs
// uBlock Origin Lite: https://github.com/uBlockOrigin/uBOL-home (GPL-3.0)
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const VERSION = "2026.1021.1354";
const SHA256 = "a12cf7dbb7021fc4890c136c97ee7493f405c637ff67acab5a3a537c602e8ab5";
const URL = `https://github.com/uBlockOrigin/uBOL-home/releases/download/${VERSION}/uBOLite_${VERSION}.edge.zip`;

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const target = join(root, "app", "src-tauri", "extensions", "ubolite");

const res = await fetch(URL);
if (!res.ok) throw new Error(`download failed: ${res.status}`);
const zip = Buffer.from(await res.arrayBuffer());
const hash = createHash("sha256").update(zip).digest("hex");
if (hash !== SHA256) throw new Error(`checksum mismatch: got ${hash}`);

const tmp = join(tmpdir(), `ubol-${VERSION}.zip`);
writeFileSync(tmp, zip);
rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
// tar on Windows 10+ unpacks zip files.
execFileSync("tar", ["-xf", tmp, "-C", target]);
console.log(`uBlock Origin Lite ${VERSION} ready in ${target}`);
