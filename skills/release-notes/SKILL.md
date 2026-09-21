---
name: release-notes
description: "Generate or update a Satsuma GitHub Release: reads the diff since the last tag (or the whole history if there is none), classifies the changes per AGENTS.md's Versioning section, infers the next semantic version if one wasn't given, fills in .github/RELEASE_TEMPLATE.md, and creates or updates the git tag and GitHub Release. Use when asked to cut a release, prepare release notes, or release a specific version."
---

# Release notes generation

Agent-agnostic: every step below is a plain `git`/`gh`/`npm` shell command. Any
coding agent with a shell and the `gh` CLI (authenticated against this repo)
can follow it — nothing here depends on a specific tool-calling interface.
The YAML frontmatter above is only there for tools (like Claude Code) that
scan for it; ignore it if your harness doesn't use that convention and just
follow the instructions below.

Run every command from the repository root.

## Usage

```
release-notes            # infer the next version from the changes since the last tag
release-notes v1.2.0      # release that exact version instead of inferring one
release-notes 1.2.0       # the "v" prefix is optional on input; normalize it back on
```

## Steps

1. **Read the template.** Load `.github/RELEASE_TEMPLATE.md` — its section
   headings (Highlights, Features, Fixes, Breaking Changes, Known Issues,
   Artifacts) are what you fill in below, verbatim. Its own HTML comments
   explain which sections to drop when empty.

2. **Determine the diff range.**
   ```bash
   LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || true)
   if [ -n "$LAST_TAG" ]; then
     RANGE="$LAST_TAG..HEAD"
   else
     # No prior tag: the range is the entire history.
     RANGE=$(git rev-list --max-parents=0 HEAD | tail -1)..HEAD
   fi
   git log --format='%H %s' "$RANGE"
   ```

3. **Gather the changes in that range.** Read both the commit subjects
   *and* the actual diff (`git diff "$RANGE"` / `git show <sha>` for
   individual commits) — a commit message alone ("fix bug") isn't enough to
   tell whether something is user-visible. If `gh` is installed and
   authenticated, cross-reference merged PRs for links and titles
   (`gh pr list --state merged --limit 100`, or `gh pr view <number>` for a
   specific one); this is a nice-to-have for linking, not a requirement —
   fall back to commit subjects alone if `gh` isn't available or isn't
   authenticated.

4. **Classify each change** into Features / Fixes / Breaking Changes, using
   AGENTS.md's "Versioning" section definitions:
   - A new, backward-compatible capability (new format/tool, new wedge
     option, new setting) → **Feature**.
   - A correction to existing behavior with no user action required →
     **Fix**.
   - Anything requiring user action, or that changes previously-relied-on
     behavior (dropped format/tool, changed `--mode=formats|tools <path>`
     CLI contract, non-backward-compatible config/theme migration) →
     **Breaking Change**.
   - Pure-internal changes (CI, tests, docs, refactors, dependency bumps
     with no behavior change) don't need a bullet in the notes at all — see
     step 6 — but still note internally whether any exist, since "nothing
     user-visible changed" still needs a PATCH bump in step 5, not no bump.

5. **Determine the target version.** The git tag is the one source of
   truth for the shipped version (see AGENTS.md's Versioning section) —
   this skill only ever creates/updates that tag, it never edits
   `package.json`/`Cargo.toml`/`tauri.conf.json` (`.github/workflows/deploy.yml`
   stamps the tag's version into those files itself, at build time, and
   never commits the result — see `scripts/set-version.mjs`).
   - If a version was given as an argument, normalize it to a `v`-prefixed
     tag name (`1.2.0` → `v1.2.0`) and use it directly — skip inference.
   - Otherwise, take the current version from `$LAST_TAG` (step 2), or
     `v0.0.0` if there was no prior tag at all (nothing has shipped yet),
     and bump it against what step 4 found:
     - Any Breaking Change found *and* the current major version is `0`
       (pre-1.0) → bump **MINOR**, per AGENTS.md (breaking changes stay
       MINOR until the project reaches 1.0).
     - Any Breaking Change found *and* the current major version is `1+`
       → bump **MAJOR**.
     - No Breaking Change, but at least one Feature found → bump **MINOR**.
     - Only Fixes, or no user-visible changes at all → bump **PATCH**
       (still bump — every release needs a version distinct from the last).
   - If the version needs a prerelease suffix (`vX.Y.Z-<prerelease>`) — e.g.
     a throwaway tag for testing `deploy.yml` itself rather than a real
     release — the `<prerelease>` identifier must be **numeric-only and no
     greater than 65535** (`v0.0.1-1`, not `v0.0.1-test` or `v0.0.1-rc.1`).
     `deploy.yml`'s Windows job bundles an MSI via WiX, whose own version
     scheme rejects any other shape for that field; a non-numeric
     prerelease still builds fine on Linux but fails `build-windows`
     outright (`optional pre-release identifier in app version must be
     numeric-only and cannot be greater than 65535 for msi target`), which
     means `publish` never runs since it's gated on both platform builds
     succeeding.

6. **Fill in the notes.** Copy the template, substitute the resolved
   version into the title, write the Highlights prose and the
   Features/Fixes/Breaking Changes/Known Issues bullets from step 4's
   classification (one bullet per change, linking its PR if you found one
   in step 3), and delete any of those four sections that end up with no
   bullets — don't leave an empty heading. Leave the "Artifacts" heading in
   place with nothing under it; `.github/workflows/deploy.yml` appends to
   it once the installers are built. Save the result to a temp file (e.g.
   `/tmp/release-notes-vX.Y.Z.md`) — it gets passed to `gh` in step 7, not
   committed to the repo as its own file.

7. **Create or update the tag and release.**
   ```bash
   TAG_EXISTS=$(git tag -l "$VERSION")
   RELEASE_EXISTS=$(gh release view "$VERSION" >/dev/null 2>&1 && echo yes || echo no)
   ```
   - **Neither exists:** tag the current commit (whatever's being
     released — typically `main`'s tip) and push it — this is what
     triggers `.github/workflows/deploy.yml` to build the installers and
     stamp this version into them — then create the release from the
     notes file:
     ```bash
     git tag "$VERSION" && git push origin "$VERSION"
     gh release create "$VERSION" --title "Satsuma $VERSION" --notes-file /tmp/release-notes-"$VERSION".md
     ```
   - **A release already exists for this version:** update it in place
     instead of creating a duplicate:
     ```bash
     gh release edit "$VERSION" --notes-file /tmp/release-notes-"$VERSION".md
     ```
     Only move the tag itself (`git tag -f "$VERSION" && git push --force origin "$VERSION"`)
     if you were explicitly asked to repoint this version at a different
     commit — that's a force-push-shaped operation on a shared ref and
     needs explicit sign-off first, not something to do as a routine part
     of "updating in place".
   - **A tag exists but no release:** create the release against the
     existing tag (same `gh release create` command as above — don't
     create a second tag).

8. **Confirm before the side-effecting steps.** Pushing a tag, creating or
   editing a GitHub Release, and (especially) force-moving an already-
   pushed tag are all visible changes to a shared repository, not local
   edits — check with whoever is directing you before running the `git
   push`/`gh release` commands in step 7, the same as you would before any
   other push or publish action.
