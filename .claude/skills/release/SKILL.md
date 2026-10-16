---
name: release
description: >
  Cuts a new iced-markdown-editor release with scripts/release.ps1: checks the [Unreleased]
  entries in CHANGELOG.md, picks the version, and (with explicit confirmation) runs the script,
  which bumps Cargo.toml, promotes the changelog section, commits, tags and pushes. The pushed
  tag makes GitHub Actions publish the crate to crates.io; no GitHub release is created. Use
  when the user says "cut a release", "release vX.Y.Z", "release candidate", "bump the version",
  or invokes "/release".
---

# Release

Releases go through `scripts/release.ps1` (Windows) or `scripts/release.zsh` (macOS, Linux);
both behave the same. Never write your own release script, never edit `Cargo.toml`'s version or
the changelog's version headings by hand, and never create or push tags with plain `git`
commands: the script keeps changelog, `Cargo.toml`, `Cargo.lock`, commit and tag consistent.

## What happens on a release

1. The script renames `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] - <today>`, adds a new
   empty `[Unreleased]` and the compare links, sets `Cargo.toml` to `X.Y.Z`, refreshes
   `Cargo.lock`, commits `Release vX.Y.Z` and pushes the branch.
2. It creates the annotated tag `vX.Y.Z` (message = the changelog section) and pushes it.
3. `.github/workflows/publish.yml` runs on the tag: checks that the tag matches `Cargo.toml`'s
   version, runs `cargo test --all-features` and `cargo publish --dry-run`, then publishes to
   crates.io. It does **not** create a GitHub release.

A release candidate (`rc`) works the same with `X.Y.Z-rc.N`: its own `[X.Y.Z-rc.N]` section,
`Cargo.toml` at `X.Y.Z-rc.N`, tag `vX.Y.Z-rc.N`, and it is published to crates.io as a
pre-release version. `new` later collects the entries of all candidates into `[X.Y.Z]`.

A version on crates.io is permanent: it can be yanked (`cargo yank --version X.Y.Z`) but never
deleted or published again. `rollback` and `retag` only help when the workflow failed before
`cargo publish`.

## Steps

1. **Check the changelog.** Read `## [Unreleased]` in `CHANGELOG.md`. If it is empty or misses
   user-visible changes since the last tag (`git describe --tags --abbrev=0 --match "v*"`), use
   the `changelog` skill to write the entries first. The script refuses to release without
   entries.

2. **Check the working tree.** `git status --short` should be clean apart from `CHANGELOG.md`;
   other uncommitted changes are not part of the release. The branch must not be behind its
   remote. Run `cargo test --all-features` locally so the workflow does not fail after the tag is
   pushed.

3. **Pick the command and version.** Without an argument the script bumps patch (only `Fixed` /
   `Security`) or minor (`Added`, `Changed`, `Deprecated`, `Removed`, `BREAKING`), never major.
   While `Cargo.toml` is at a candidate `X.Y.Z-rc.N`, it continues with `X.Y.Z`.
   - Release: `.\scripts\release.ps1 new` (or `new patch|minor|major|X.Y.Z`)
   - Release candidate: `.\scripts\release.ps1 rc` (or `rc patch|minor|major|X.Y.Z`)
   - For the version it would pick, `.\scripts\release.ps1 new -h` explains the rules; the script
     prints the plan (tag, version reason, changelog and `Cargo.toml` changes, tag message)
     before it asks for confirmation.

   Breaking changes in a 0.x crate are a minor bump; from 1.0 on they need `major`. Ask the user
   if the choice is not obvious.

4. **Confirm, then run.** Running the script commits, pushes and publishes to crates.io, which
   cannot be undone. Show the user the `[Unreleased]` section and the version you expect, and ask
   explicitly before running it — even if earlier steps were approved. Then run it with `-Yes`
   (`--yes` for the zsh script), since the session cannot answer its prompt.

5. **Report.** Give the tag and the link to the workflow run
   (`https://github.com/iocast/iced-markdown-editor/actions/workflows/publish.yml`) and the crate
   page (`https://crates.io/crates/iced-markdown-editor`).

## If the publish workflow fails

- Failed before `cargo publish` (tests, version check): fix the cause, commit, then either
  `.\scripts\release.ps1 retag vX.Y.Z -Yes` to move the tag to the fix, or
  `.\scripts\release.ps1 rollback vX.Y.Z -Yes` and release again. Ask before either: both
  rewrite a pushed tag.
- Failed at or after `cargo publish`: check crates.io. If the version is there, it is released;
  do not retag. A broken published version gets yanked and fixed in the next patch release.
