#!/usr/bin/env node
// Merges one or more `rustdoc --show-coverage --output-format json` files
// (one per workspace crate - rustdoc only supports documenting a single
// crate's coverage per invocation) into a single shields.io endpoint JSON
// (https://shields.io/badges/endpoint-badge), published as
// docs-coverage-badge.json alongside the deployed docs site.
//
// Usage: node scripts/docs-coverage-badge.mjs <out.json> <coverage1.json> [coverage2.json ...]

import { readFileSync, writeFileSync } from "node:fs";

const [outPath, ...coveragePaths] = process.argv.slice(2);

if (!outPath || coveragePaths.length === 0) {
  console.error("usage: docs-coverage-badge.mjs <out.json> <coverage1.json> [coverage2.json ...]");
  process.exit(1);
}

let totalItems = 0;
let documentedItems = 0;

for (const path of coveragePaths) {
  const perFile = JSON.parse(readFileSync(path, "utf8"));
  for (const { total, with_docs } of Object.values(perFile)) {
    totalItems += total;
    documentedItems += with_docs;
  }
}

if (totalItems === 0) {
  console.error("no documentable items found across the given coverage files");
  process.exit(1);
}

const pct = (documentedItems / totalItems) * 100;
// shields.io convention: brighter green the closer to full coverage.
const color =
  pct >= 90 ? "brightgreen" : pct >= 75 ? "green" : pct >= 60 ? "yellowgreen" : pct >= 40 ? "yellow" : pct >= 20 ? "orange" : "red";

const badge = {
  schemaVersion: 1,
  label: "docs",
  message: `${pct.toFixed(1)}%`,
  color,
};

writeFileSync(outPath, JSON.stringify(badge));
console.log(`docs coverage: ${documentedItems}/${totalItems} (${badge.message}) -> ${outPath}`);
