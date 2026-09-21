# Contributing to Satsuma

Thanks for taking a look at Satsuma. This is a pre-1.0 project under active development (see [Current status](README.md#current-status) for exactly what's real today vs. still planned), so contributions of all sizes are useful: bug reports, docs fixes, tests, and code.

Before diving in:

- [README.md](README.md) — project concept, target format/tool scope, architecture, and dev commands
- [AGENTS.md](AGENTS.md) — repo conventions: build/test commands, the Cargo workspace layout, and (importantly) the rule that a feature touching the overlay/wedge-menu trigger pipeline needs both unit tests *and* real-desktop e2e tests, not just one

## AI-assisted contributions

Satsuma's own codebase is developed with AI coding assistance (Claude Code) — see `AGENTS.md`/`CLAUDE.md` in this repo. So AI-assisted contributions from others are welcome too, on the same terms:

- **Disclose it.** Say in the PR description which parts were AI-assisted and with what tool. This isn't a mark against the PR — it just tells reviewers what to double-check.
- **You own the review.** Before submitting, you should personally understand, have tested, and be able to explain every line — the same standard as hand-written code. Don't submit AI output you haven't verified yourself.
- **Same bar either way.** AI-assisted and hand-written PRs are reviewed against the same standard: does it work, is it tested, does it fit the architecture. "An AI wrote it" isn't a defense for an untested or unexplained change.

## Getting set up

```bash
npm install
scripts/fetch-ffmpeg.sh && scripts/fetch-pdfium.sh   # required once per checkout — see README
npm run tauri dev     # launches the app (Rust + webview)
```

See [README.md#development](README.md#development) for the full command list, including the Linux tray-icon dependency and the Windows-cross-check-without-Windows setup.

## Testing

Every change should be verifiable with the existing test suites:

```bash
npm run test    # frontend: Vitest
cargo test      # backend: Rust unit tests (workspace-wide)
```

If your change touches the overlay/wedge-menu trigger-to-selection pipeline (a new wedge option, a new mode, changes to `show_overlay`/`hide_overlay`/`LaunchRequest` handling, a new OS-integration entry point), also extend the real-desktop e2e harnesses in `tests/e2e-linux/` and `tests/e2e-windows/` — see [AGENTS.md](AGENTS.md) for why unit tests structurally can't reach that path, and for how to run them locally.

## Submitting a PR

- Fill out the PR template — the checklist at the end is what reviewers check first.
- CI (`.github/workflows/ci.yml`) must pass: frontend/backend unit tests, both e2e jobs, and the Windows type-check.
- Keep PRs focused. A bug fix doesn't need an unrelated refactor riding along with it.

## Reporting bugs

Use the bug report issue template — it asks for your OS, how you triggered the wedge menu (drag-and-drop vs. a file-manager trigger), and the commit you built from, all of which matter for a project this platform-dependent.
