---
name: pr-review-loop
description: "Run a two-agent PR review-and-fix team: a REVIEWER on its own read-only worktree reviews the whole pull request and sends numbered, severity-tagged comments ([blocker]/[major]/[minor]/[nit]) to a FIXER on the PR branch, who fixes them with CI green as top priority; the loop repeats (review -> fix -> CI green -> re-review) until the reviewer signals REVIEW_STATUS: done (no issues or only trivial nits), the fixer applies the final batch, and the loop stops. Use when asked to review a pull request with a fixing agent, run a two-agent review/fix loop, or continue a team review already in progress."
---

# Two-agent PR review and fixing loop

A review model for pull requests that need a by-reviewer's whole-PR pass
*and* a fixer's turn-around, run in a loop until both sides call it done.
One orchestrator drives two coding-agent roles; the only one that edits is
the fixer.

- **Orchestrator** (you, if you're running this): creates/enters both
  worktrees, relays comments verbatim, independently verifies CI and branch
  state, enforces the contracts below, and decides when the loop ends.
- **Reviewer**: reviews the ENTIRE PR freshly each round from its own
  read-only, detached worktree. Never edits, never pushes, never posts on
  the PR.
- **Fixer**: the only agent that edits. Works in the PR-branch worktree.

This skill is the playbook all three follow; it's written agent-agnostically
so any coding agent with a shell and `git`/`gh` can be dropped into either
role. The YAML frontmatter above is only for tools that scan for it; ignore
it if your harness doesn't use that convention and just follow the steps.
Project-specific values (commit trailer, knowledge graph, local test gates,
CI job names) live in "Project knobs" at the bottom — fill those in per repo
before starting.

## Setup

- Create two worktrees of the same repository:
  - **reviewer worktree** — checked out *detached* at `origin/<branch>` and
    refreshed each round (`git fetch origin && git checkout --detach
    origin/<branch>` there). Kept read-only; it must never gain commits.
  - **fixer worktree** — the PR branch checked out, synced with origin.
- Prerequisites: `gh` CLI authenticated against the repo; CI configured on
  the branch (the loop's green signal is the *real* CI suite — local tests
  alone don't count). If CI doesn't exist, say so up front and run the loop
  on local gates only, with that limitation stated.

## The loop contract (order of operations)

1. **Reviewer reviews the whole PR** (see Reviewer protocol) and ends with a
   verdict line.
2. **CI first, always.** If any CI job is red at any point in the loop, the
   fixer fixes CI before touching anything else — including before addressing
   reviewer comments. A red suite stalls the loop; reviewer comments wait.
3. Reviewer comments (if any) are then addressed by the fixer, in numbered
   order, CI-green-first.
4. **The orchestrator verifies independently** that the whole suite is green
   on the new head before signaling the reviewer for the next round. Never
   take the fixer's "it's green" on faith — check `gh run view <id> --json
   jobs --jq '.jobs[] | {name, conclusion}'`.
5. Re-review: the reviewer refreshes its detached checkout and re-reviews the
   *cumulative* PR, re-verifying old comments and checking the new fix round.
6. Repeat until the **termination contract** fires.

## Reviewer protocol

- Each round: `git fetch origin && git checkout --detach origin/<branch>`
  inside the reviewer worktree. Never commit, push, or create branches.
- Review the cumulative diff since the last review round; the first round
  covers the whole PR against its base.
- Comments are a **numbered list addressed to the fixer**, each tagged
  exactly `[blocker]` / `[major]` / `[minor]` / `[nit]`, with:
  `file:line` of the problem, a description of why it matters, and the
  *specific* fix you're requesting.
- Re-verify previous comments are genuinely addressed (not papered over),
  and review the new additions from the fix round itself. In particular,
  look for the **twin bug**: the same failure class as whatever the fix just
  addressed, appearing in a sibling code path (e.g. a Rust-side
  stale-async-continuation race fixed in the backend often has a frontend
  twin). This pattern has repeatedly caught real second instances.
- End your report with exactly one of these two lines, nothing else after:
  - `REVIEW_STATUS: done` — no issues, or only extremely-low-severity
    leftovers (see Termination).
  - `REVIEW_STATUS: needs-work` — you have at least one `[blocker]`/`[major]`
    (or a genuinely valuable `[minor]`) finding that justifies another round.

## Fixer protocol

- Work only in the fixer worktree, on the PR branch.
- Priority order, strictly: (1) the user's explicit task, (2) CI red fixes,
  (3) reviewer comments.
- Run the repo's local gates before pushing (see Project knobs).
- Commit rules (non-negotiable):
  - Commit message ends with the project's `Co-Authored-By:` trailer.
  - Any knowledge-graph/derived-artifact regeneration (e.g. `graphify update .`)
    is folded into the **same commit** as the code — never a separate commit.
  - Before every push: `git fetch origin`; if origin moved, **rebase** onto
    `origin/<branch>`. Never force-push; never clobber commits you didn't
    create; leave other sessions'/users' worktrees untouched.
  - **Commit and push before reporting.** A lost or empty report must never
    lose work; if the harness has a history of dropping an agent's final
    response, treat "the branch state after I return" as your only
    deliverable and never leave work uncommitted.
- CI iteration: push → find the run for your new head sha (`gh run list
  --branch <branch>` / `gh run watch <id>`) → fix → commit → push → repeat
  *internally* until the ENTIRE suite is green. Only then return.

## Orchestrator protocol

- Relay reviewer comments to the fixer **verbatim** — numbered, tagged,
  unedited. Never interpret on the fixer's behalf.
- Before each reviewer signal, independently verify the fixer's claims:
  - Branch state: `git fetch origin; git rev-parse HEAD origin/<branch>`
    (equal), worktree clean.
  - Trailer on the new head: `git show -s --format='%(trailers:key=Co-Authored-By,valueonly)' HEAD`.
  - CI: `gh run view <id> --json jobs --jq '.jobs[] | {name, conclusion}'`
    — every job green on the head sha.
- **Silent-subagent failure mode**: sessions sometimes return empty reports
  while their work (or half-finished WIP) lands in the worktree. Handle it by
  checking the branch/worktree directly (`git log`, `git status`, `git diff`)
  and recovering WIP; if the same session keeps going silent, start a *fresh*
  session with full context and the commit-before-return rule, rather than
  continuing the wedged one.
- **Flaky-last-job rule**: when the only red job is flaky (passes some runs,
  fails others on near-identical heads), demand a root-cause fix, not a
  timeout bump. Have the fixer (a) add diagnostics that make the failure
  actionable (process/window dumps, log dumps), (b) use the pass/fail history
  across runs to argue race vs. flake, and (c) fix the mechanism. Timeout
  bumps paper over the race and the loop keeps re-tripping.
- **Base-stability rule**: if another session has a checkout based on the
  branch's base commit, keep that base commit stable — history rewrites
  (even byte-identical tree rewrites like trailer fixes) invalidate their
  checkout. If a rewrite is unavoidable, keep `main` base commits unchanged
  and announce it.
- Consider backing up the branch locally (`git branch backup-...`) before a
  risky operation; never push the backup.

## Termination contract

- The reviewer signals `done` when it finds **no issues**, or everything
  remaining is extremely low severity (nits/gratuitous polish not worth
  another round-trip). The cycle must not run forever.
- On `done`: the fixer applies the last small batch (the optional polish and
  any trivial leftovers), verifies the suite stays green, and **stops**.
  There are **no further re-review rounds** after `done`.
- On `needs-work`: the loop continues, but only when a real finding justifies
  it — don't prolong the cycle with manufactured comments.

## Definition of done

- The reviewer's last verdict was `done`.
- The whole CI suite is green on the final head.
- Branch state verified: `HEAD == origin`, correct trailer, worktree clean.
- The loop has stopped — no rounds still running in the background.

## Project knobs

Fill these in per repository before starting. Satsuma's values (this repo):

- **Commit trailer**: `Co-Authored-By: Big Pickle <noreply@anoma.ly>`
  (exact — a one-character typo in this trailer has already forced a history
  rewrite once).
- **Knowledge graph**: `graphify update .` after code edits, folded into the
  same commit (never a separate graphify commit).
- **Local gates**: `cargo test` (workspace), `npm run test`,
  `npx tsc --noEmit`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `npm run test:coverage` (85% branch gate).
- **CI surface**: real-desktop e2e jobs (`e2e-linux`, `e2e-windows`) run in
  CI only — the Windows harness cannot run on a Linux fixer machine, so
  Windows-only scenarios are blind-debugged remotely (push → wait ~10 min →
  read logs → new hypothesis). That long feedback loop is expected; it is the
  main reason rounds look like "constant rerunning".
- **Test-track rule** (AGENTS.md): features that touch the
  overlay/wedge-menu trigger-to-selection pipeline need scenarios in BOTH
  e2e harnesses, not just unit tests; pure-backend logic needs unit tests
  only.