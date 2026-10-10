---
name: changelog
description: Generate or update entries in the [Unreleased] section of CHANGELOG.md (Keep a Changelog + Semantic Versioning) from git history, uncommitted changes, or the user's own description. Use this whenever the user wants to add, write, update, reword, move, or remove a changelog entry, document or summarize what changed since the last release, prepare release notes, or get ready for a release or test build with scripts/release.ps1 (rc / new). Also use it when the user wraps up a feature, fix, or breaking change and asks to "document it", even if the word changelog is not mentioned.
allowed-tools: Read, Edit, Write, Grep, Glob, Bash(git log:*), Bash(git diff:*), Bash(git status:*), Bash(git describe:*), Bash(git tag:*), Bash(git show:*)
---

# Changelog entries

This project releases with `scripts/release.ps1`, which reads `CHANGELOG.md`:

- The tag message and the GitLab release text are copied from the changelog, so every entry becomes published release notes.
- Without an explicit argument, the script picks patch or minor from the section headings you use. A wrong heading means a wrong version. The major number only changes when the user runs `new major`.

Accuracy and correct classification matter more than listing every commit.

## Scope

- Edit only the `## [Unreleased]` section. Released sections (`## [X.Y.Z] - date`) are history. Change them only if the user explicitly asks, for example to fix a typo.
- Never add version headings, dates, or compare links at the bottom of the file. `release.ps1 new` creates those when releasing.
- Do not commit, tag, or run the release script unless the user asks for it.
- Keep the rest of the file exactly as it is: line endings, blank lines, link references.

## Workflow

### 1. Read the current state

Read `CHANGELOG.md` in the repository root. If it does not exist, create it with the same skeleton that `release.ps1 init` produces:

```markdown
# Changelog

All notable changes to this project are documented in this file.
Format: Keep a Changelog (https://keepachangelog.com), versioning: Semantic Versioning (https://semver.org).

## [Unreleased]

### Added

### Changed

### Fixed
```

Find the latest real release. Release candidates (`-rc.N`) do not count, because their notes are still in `[Unreleased]`:

```bash
git describe --tags --abbrev=0 --match "v*" --exclude "*-rc.*"
```

If this fails, there is no release yet. In that case, treat the whole history as unreleased and focus on the most recent work.

### 2. Collect the changes

If the user describes the changes, that description is the primary source. Use git only to fill gaps and to check details. Otherwise, gather the changes like this:

```bash
git log <last-tag>..HEAD --no-merges --format="%h %s%n%b"
git diff --stat <last-tag>..HEAD
git status --short
```

Commit subjects are often vague ("fix", "wip", "update"). For anything that looks user-visible, read the actual diff of the relevant files (`git diff <last-tag>..HEAD -- <path>`). Describe the real effect, not the commit wording.

Include uncommitted changes (`git diff HEAD`) only when the user is documenting their current work.

### 3. Decide what belongs in the changelog

The readers are people who deploy, operate, or use this service. Include what they notice:

- new features, endpoints, CLI flags
- changed behavior, defaults, response formats
- new, renamed, or removed configuration: environment variables, config keys, Helm values, ports, volumes
- database migrations or other steps needed during deployment
- bug fixes for bugs that existed in a released version
- security fixes, including dependency or base-image updates made for security reasons

Leave out purely internal work: refactoring without behavior change, tests, formatting, CI tweaks, internal docs. The exception is internal work that affects building or deploying, such as a new required build argument or a changed image base.

**Same-cycle rule:** if a bug was introduced and fixed within the current `[Unreleased]` cycle, nobody outside ever saw it. Do not add a `Fixed` entry for it. Adjust the original `Added` or `Changed` entry instead, if needed.

### 4. Classify

Use these headings, in this order:

| Heading          | Use for                                     | Automatic bump |
|------------------|---------------------------------------------|----------------|
| `### Added`      | new features                                | minor          |
| `### Changed`    | changes to existing behavior                | minor          |
| `### Deprecated` | still works, will be removed later          | minor          |
| `### Removed`    | removed features, endpoints, config options | minor + warning |
| `### Fixed`      | bug fixes                                   | patch          |
| `### Security`   | vulnerability fixes                         | patch          |

The script takes the highest effect found in `[Unreleased]`. The automatic bump never goes beyond minor; a major release needs `.\scripts\release.ps1 new major`. Classify carefully:

- **Breaking changes:** prefix the entry with `**BREAKING:**`, usually under `Changed`. The script warns when it finds the uppercase word `BREAKING` or `### Removed` entries, so the user can decide on `major`. Never write that word in uppercase for anything that is not a real break.
- **Removed:** use it only for things consumers relied on. Deleting internal code does not belong there.

Add missing headings in the order shown above. Leave empty skeleton headings in place; the release script drops them when releasing.

### 5. Write the entries

- One bullet (`- `) per change. Keep it to one line where possible, two at most.
- Describe the effect from the reader's point of view, not the implementation.
  - Good: `- Uploads larger than 10 MB no longer time out`
  - Bad: `- Refactor UploadService to use streaming`
- Put concrete names in backticks: endpoints, env vars, config keys, flags.
- For breaking changes, removals, and config changes, say what the reader has to do. Example: `set \`LOG_LEVEL\` instead of \`LOG_LVL\``.
- Keep issue or merge request references from the commits (`#123`, `!45`) at the end of the entry. Leave out commit hashes and author names.
- Match the language, tense, and style of the existing entries. If there are none, write short English sentences starting with a capital letter.

### 6. Update instead of duplicating

- Before adding an entry, check whether `[Unreleased]` already covers the change. If it does, extend or reword that entry instead of adding a second one.
- When the user asks to reword, move, merge, or remove a specific entry, do exactly that and leave the other entries untouched.
- If a later commit changes something that already has an entry, update the entry to the final state. An example is a new option that was renamed before release.

### 7. Report back

After editing, show the user:

1. The resulting `[Unreleased]` section.
2. The predicted next version: the latest release tag plus the highest effect from the table above. Example: `v1.3.0 -> v1.3.1 (only fixes)`. If there are breaking changes or removals, say so and mention that `new major` is needed for a new major version.
3. The changes you deliberately left out, briefly, so the user can ask to include them.
4. The next step: `.\scripts\release.ps1 rc` for a test build or `.\scripts\release.ps1 new` for the release. Add `patch`, `minor` or `major` to choose the number explicitly.

## Example

**Commits since `v1.3.0`:**

```
a1b2c3d add /healthz endpoint
b2c3d4e fix healthz returning 500 when db is down
c3d4e5f rename env LOG_LVL to LOG_LEVEL
d4e5f6a refactor db connection pool
e5f6a7b bump alpine base image for openssl security fixes (#88)
```

**Result:**

```markdown
## [Unreleased]

### Added
- `/healthz` endpoint for Kubernetes liveness and readiness probes; reports unhealthy while the database is unreachable

### Changed
- **BREAKING:** Environment variable `LOG_LVL` was renamed to `LOG_LEVEL`; update your deployment manifests

### Security
- Updated the Alpine base image to include current OpenSSL security fixes (#88)
```

**Report:**

- Next version: `v1.3.0 -> v1.4.0` (automatic). The BREAKING entry is a candidate for `.\scripts\release.ps1 new major` -> `v2.0.0`.
- `b2c3d4e` was folded into the `/healthz` entry, since the bug never reached a release.
- `d4e5f6a` was left out as an internal refactor.
