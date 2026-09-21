---
description: Generate or update a Satsuma GitHub Release
---

Load the `release-notes` skill (skill id `release-notes`; source file at
`skills/release-notes/SKILL.md`) and follow it exactly. Use the skill tool
if you have one, otherwise read the file — don't re-type its steps here.

Anything typed after the command name is an optional target version to
release instead of inferring one: $ARGUMENTS

Run every command from the repository root, and confirm before the
side-effecting steps (pushing the tag, creating/editing the GitHub Release)
as the skill instructs.