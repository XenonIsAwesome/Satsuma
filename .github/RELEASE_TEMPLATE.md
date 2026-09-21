<!--
  Release notes template for Satsuma.

  Filled in by the release-notes-generation skill (skills/release-notes/SKILL.md)
  from the diff since the last tag, then attached as the body of the GitHub
  Release for the version tag. The "Artifacts" section at the end is appended
  separately, by .github/workflows/deploy.yml, once the Windows and Linux
  installers are actually built - leave that heading in place (even with no
  content under it yet) so the workflow has something to append below.

  Delete any other section that ends up empty (e.g. skip "Breaking Changes"
  entirely for a patch release with none) rather than leaving an empty
  heading. See AGENTS.md's "Versioning" section for how changes are
  classified into these sections and against major/minor/patch.
-->

# Satsuma vX.Y.Z

## Highlights

<!--
  1-3 sentences of prose on the one or two things a user would most want to
  know about this release. Skip this section for a routine patch release
  with nothing highlight-worthy - not every release needs a highlight reel.
-->

## Features

<!--
  New, backward-compatible functionality. One bullet per change, PR-linked:
  - Short, user-facing description of the feature. ([#123](../../pull/123))
-->

## Fixes

<!--
  Bug fixes and other corrections to existing behavior:
  - Short, user-facing description of the fix. ([#123](../../pull/123))
-->

## Breaking Changes

<!--
  Anything that requires user action, or changes previously-relied-on
  behavior (a dropped format/tool, a changed file-manager trigger contract,
  a config/theme format migration that isn't backward compatible, ...).
-->

## Known Issues

<!--
  Real, outstanding problems a user should know about before installing
  this release - not a general disclaimer.
-->

## Artifacts

<!-- Filled in by .github/workflows/deploy.yml after the installers are built - do not fill in manually. -->
