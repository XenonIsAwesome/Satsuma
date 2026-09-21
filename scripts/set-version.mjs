#!/usr/bin/env node
// Stamps a version into every place Satsuma's version is declared -
// package.json, src-tauri/tauri.conf.json, and the [package] version of
// src-tauri/Cargo.toml and crates/satsuma-core/Cargo.toml - so the git tag
// (see .github/workflows/deploy.yml, which strips the tag's leading "v"
// and passes the result here right before building) is the one place that
// actually determines the shipped version. Never run against a working
// tree you intend to commit from: deploy.yml runs this in an ephemeral CI
// checkout and never commits the result back - see AGENTS.md's
// "Versioning" section.
//
// Usage:
//   node scripts/set-version.mjs <version>   # e.g. 1.2.0 - no leading "v"

import { readFileSync, writeFileSync } from "node:fs";

const [version] = process.argv.slice(2);

if (!version) {
  console.error("usage: set-version.mjs <version>");
  process.exit(1);
}

// Same semver shape package.json/Cargo.toml/tauri.conf.json all expect -
// MAJOR.MINOR.PATCH with optional -prerelease/+build metadata.
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;
if (!SEMVER.test(version)) {
  console.error(`"${version}" doesn't look like a bare semver version (no leading "v"?) - expected e.g. "1.2.0"`);
  process.exit(1);
}

function setJsonVersion(path) {
  const text = readFileSync(path, "utf8");
  const trailingNewline = text.endsWith("\n");
  const data = JSON.parse(text);
  data.version = version;
  const out = JSON.stringify(data, null, 2) + (trailingNewline ? "\n" : "");
  writeFileSync(path, out);
  console.log(`${path}: version -> ${version}`);
}

// Only the [package] table's own `version = "..."` line - a dependency
// pin never appears as a bare `version = "..."` at the start of a line
// (it's always `name = "x"` or nested inside `{ version = "x", ... }`),
// so matching that shape is enough to avoid touching anything else.
function setCargoTomlVersion(path) {
  const text = readFileSync(path, "utf8");
  const pattern = /^version = ".*"$/m;
  if (!pattern.test(text)) {
    console.error(`${path}: no top-level "version = ..." line found`);
    process.exit(1);
  }
  writeFileSync(path, text.replace(pattern, `version = "${version}"`));
  console.log(`${path}: version -> ${version}`);
}

setJsonVersion("package.json");
setJsonVersion("src-tauri/tauri.conf.json");
setCargoTomlVersion("src-tauri/Cargo.toml");
setCargoTomlVersion("crates/satsuma-core/Cargo.toml");
