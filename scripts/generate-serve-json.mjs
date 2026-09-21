#!/usr/bin/env node
// Generates site/serve.json for local preview (`npx serve site`, see
// .claude/launch.json) from the ACTUAL assembled site/ tree, rather than
// hand-maintaining a fixed list or a clever regex.
//
// Why this exists: rustdoc's own pages are directory-style
// (satsuma_core/index.html) while TypeDoc's are flat files
// (modules/X.html) - `serve`'s `trailingSlash`/`cleanUrls` options are
// global booleans that can't express "directory-style here, flat-file
// there" on their own, and neither `redirects` nor `rewrites` entries can
// safely use a glob/regex for "any real subdirectory" (serve-handler's
// own glob-to-regex conversion mangles a `*` used inside a custom regex
// group, and its path-to-regexp compile() URL-encodes slashes in a
// captured multi-segment value). Enumerating the real directories here -
// each one getting one exact-match rewrite to its own index.html -
// sidesteps all of that with a mechanism serve-handler already handles
// correctly (plain string matching), and it can't drift out of sync with
// the actual rustdoc output the way a hand-written list could.
//
// Usage: node scripts/generate-serve-json.mjs <site-dir>

import { readdirSync, statSync, writeFileSync, existsSync } from "node:fs";
import { join, relative, sep } from "node:path";

const [siteDir] = process.argv.slice(2);
if (!siteDir) {
  console.error("usage: generate-serve-json.mjs <site-dir>");
  process.exit(1);
}

const rustDir = join(siteDir, "rust");

function findIndexDirs(dir, out) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const full = join(dir, entry.name);
    if (existsSync(join(full, "index.html"))) {
      out.push(full);
    }
    findIndexDirs(full, out);
  }
}

const indexDirs = [];
if (existsSync(rustDir)) {
  findIndexDirs(rustDir, indexDirs);
}

const rewrites = [
  { source: "/", destination: "/index.html" },
  { source: "/ts", destination: "/ts/index.html" },
  ...indexDirs.map((dir) => {
    // e.g. site/rust/satsuma_core/file_type -> /rust/satsuma_core/file_type
    const source = "/" + relative(siteDir, dir).split(sep).join("/");
    return { source, destination: `${source}/index.html` };
  }),
];

const serveJson = {
  trailingSlash: true,
  cleanUrls: false,
  rewrites,
};

writeFileSync(join(siteDir, "serve.json"), JSON.stringify(serveJson, null, 2) + "\n");
console.log(`generated serve.json with ${indexDirs.length} rustdoc directory rewrite(s)`);
