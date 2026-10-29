// Cuts a release: sets the version, commits, tags and pushes. GitHub Actions
// then builds the installer and publishes it, and installed copies update.
//
//   node scripts/release.mjs 0.2.0

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("Usage: node scripts/release.mjs <major.minor.patch>");
  process.exit(1);
}
const run = (cmd) => execSync(cmd, { stdio: "inherit" });
// Untracked files are fine: the release commits only the version files.
if (execSync("git status --porcelain --untracked-files=no").toString().trim()) {
  console.error("Commit or stash your changes first.");
  process.exit(1);
}
// Checked before anything changes: without a remote, the push at the end
// fails after the commit and tag are already made.
const remote = execSync("git remote").toString().trim();
if (!remote) {
  console.error("This repo has no remote to push to. Add the GitHub repo first:\n  git remote add origin https://github.com/koubayashi/Hearth.git");
  process.exit(1);
}
if (execSync(`git tag --list v${version}`).toString().trim()) {
  console.error(`Tag v${version} already exists.`);
  process.exit(1);
}

// The app's version comes from the workspace Cargo.toml (tauri.conf.json has none).
const cargo = readFileSync("Cargo.toml", "utf8");
const bumped = cargo.replace(/(\[workspace\.package\][^[]*?version = ")[^"]+"/, `$1${version}"`);
if (bumped === cargo) {
  console.error(`Cargo.toml is already at ${version}, or its version line wasn't found.`);
  process.exit(1);
}
writeFileSync("Cargo.toml", bumped);
for (const file of ["app/package.json"]) {
  const json = JSON.parse(readFileSync(file, "utf8"));
  json.version = version;
  writeFileSync(file, JSON.stringify(json, null, 2) + "\n");
}
run("cargo update --workspace --offline");

run("git add Cargo.toml Cargo.lock app/package.json");
run(`git commit -m "Hearth ${version}"`);
run(`git tag v${version}`);
run("git push");
run(`git push origin v${version}`);
console.log(`\nTagged v${version}. The release appears once the build finishes (about 15 minutes).`);
