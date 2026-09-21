#!/usr/bin/env node
// Writes a shields.io endpoint-badge JSON (https://shields.io/badges/endpoint-badge)
// for one of the analysis-CI metrics that has no off-the-shelf badge
// (coverage %, cargo-deny result, clippy warning count, audit vulnerability
// count, knip unused-item count - see the "Badges" section of
// .github/workflows/*.yml for which step calls this with which mode).
//
// Usage:
//   node scripts/generate-badge.mjs <out.json> <label> percentage <pct>
//   node scripts/generate-badge.mjs <out.json> <label> count <n>
//   node scripts/generate-badge.mjs <out.json> <label> status <pass|fail>

import { writeFileSync } from "node:fs";

const [outPath, label, mode, value] = process.argv.slice(2);

if (!outPath || !label || !mode || value === undefined) {
  console.error("usage: generate-badge.mjs <out.json> <label> <percentage|count|status> <value>");
  process.exit(1);
}

let message;
let color;

if (mode === "percentage") {
  const pct = Number(value);
  message = `${pct.toFixed(1)}%`;
  // shields.io convention: brighter green the closer to full coverage.
  color = pct >= 90 ? "brightgreen" : pct >= 75 ? "green" : pct >= 60 ? "yellowgreen" : pct >= 40 ? "yellow" : pct >= 20 ? "orange" : "red";
} else if (mode === "count") {
  const n = Number(value);
  message = `${n}`;
  // Lower-is-better: any nonzero count is at least a warning color, never green.
  color = n === 0 ? "brightgreen" : n <= 3 ? "yellow" : n <= 10 ? "orange" : "red";
} else if (mode === "status") {
  if (value !== "pass" && value !== "fail") {
    console.error(`status mode expects "pass" or "fail", got: ${value}`);
    process.exit(1);
  }
  message = value;
  color = value === "pass" ? "brightgreen" : "red";
} else {
  console.error(`unknown mode: ${mode} (expected percentage, count, or status)`);
  process.exit(1);
}

const badge = { schemaVersion: 1, label, message, color };
writeFileSync(outPath, JSON.stringify(badge));
console.log(`${label}: ${message} -> ${outPath}`);
