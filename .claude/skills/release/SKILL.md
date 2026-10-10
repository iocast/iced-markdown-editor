---
name: release
description: >
  Cuts a new iced-markdown-editor release: bumps the version consistently across Cargo.toml and
  CHANGELOG.md, rewrites the changelog entry in Keep a Changelog format, commits,
  and (with explicit confirmation) tags and pushes to trigger the GitHub Actions
  release workflow. Use when the user says "cut a release", "release vX.Y.Z",
  "bump the version", or invokes "/release".
---

Follow the release process defined in `README.md` under "Building and Releasing" and the
changelog conventions defined in `CHANGELOG.md` (Keep a Changelog + Semantic Versioning).

## Steps

1. **Determine the next version.**
   - Read `Cargo.toml`'s `[package].version` for the current version.
   - Read the unreleased changes (commits since the last version tag, or content already
     staged under `<!-- next-header -->` in `CHANGELOG.md` if present).
   - Pick the next SemVer version: `MAJOR` for breaking changes, `MINOR` for backwards-compatible
     features, `PATCH` for fixes only. Confirm the choice with the user if it's ambiguous.

2. **Update `CHANGELOG.md`.**
   - Add a new section directly below `<!-- next-header -->`:
     `## [X.Y.Z] - YYYY-MM-DD` (today's date).
   - Group entries under the standard Keep a Changelog headings, only the ones that apply:
     `### Added`, `### Changed`, `### Deprecated`, `### Removed`, `### Fixed`, `### Security`.
   - Write entries as user-facing prose describing behavior, not raw commit messages —
     match the descriptive style of existing entries (see the `[0.2.0]` entry).
   - Do not touch or reformat older version sections.

3. **Update `Cargo.toml`.**
   - Set `[package].version` to the same `X.Y.Z`. This is the field the release workflow
     checks against the changelog header, so the two **must** match exactly.
   - If `Cargo.lock` tracks the package version, regenerate it (`cargo check` is enough).

4. **Verify consistency before committing.**
   - Confirm `Cargo.toml` version == the new `CHANGELOG.md` H2 header version. The GitHub
     Actions workflow aborts the build on mismatch, so catch it locally first.

5. **Commit.**
   - Stage exactly `Cargo.toml`, `Cargo.lock` (if changed), and `CHANGELOG.md`.
   - Commit message: `Release vX.Y.Z` (matches the format shown in the README's release steps).

6. **Confirm before tagging/pushing.**
   - Tagging and pushing trigger a public GitHub Actions release build and publish binaries —
     this is a hard-to-reverse, shared-state action. Always show the user the diff/commit and
     explicitly ask before running:
     ```
     git tag vX.Y.Z
     git push
     git push origin vX.Y.Z
     ```
   - Never push or tag without explicit confirmation, even if earlier steps were pre-approved.

## Boundaries

Only prepares and (on confirmation) publishes a release exactly as documented in README.md.
Does not invent changelog categories beyond Keep a Changelog's standard six, does not bump
the version without a clear rationale from the diff/commits, and does not skip the
Cargo.toml/CHANGELOG.md consistency check the CI relies on.
