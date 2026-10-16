---
name: announce
description: Write social media announcements for an iced-markdown-editor release from its CHANGELOG.md section, in three versions - short (X), normal (Mastodon) and a Ko-fi post. Use when the user wants to announce, post about, promote or share a release, asks for a release post, tweet, toot or Ko-fi update, or invokes /announce, optionally with a version such as 0.3.1.
allowed-tools: Read, Grep, Bash(git describe:*), Bash(git tag:*), Bash(git show:*), Bash(python:*)
---

# Release announcements

Writes three ready-to-paste posts for one release. Nothing is posted: Ko-fi has no API for
posts, and publishing is the user's job.

## 1. Pick the release

- Use the version the user names (`0.3.1` or `v0.3.1`).
- Otherwise use the latest tag: `git describe --tags --abbrev=0 --match "v*"`.
- Read its section `## [X.Y.Z] - <date>` in `CHANGELOG.md`. If there is none, say so and stop;
  do not invent changes.
- For a release candidate (`X.Y.Z-rc.N`) call it a release candidate and say how to try it:
  `iced-markdown-editor = "X.Y.Z-rc.N"` (Cargo only picks a pre-release when asked for it).

## 2. Choose what to say

Changelog entries are written for developers and are too long for social media. Pick the one
or two changes users care about most (usually from Added, or a notable fix) and say them in
plain words. API names only where they help, e.g. `Language` is fine in the Ko-fi post, not
in the X post. Never claim anything the section does not say.

## 3. Write the three versions

Facts to use:

- The project: a Word-style WYSIWYG Markdown (CommonMark) editor widget for the Rust GUI
  library iced, on desktop and the web.
- Crate: https://crates.io/crates/iced-markdown-editor
- Docs: https://docs.rs/iced-markdown-editor
- Live demo: https://iocast.github.io/iced-markdown-editor/
- Repository: https://github.com/iocast/iced-markdown-editor
- Ko-fi: https://ko-fi.com/iocast

**Short (X)** - at most 280 characters as X counts them (every link counts as 23, an emoji
as 2). Version, one sentence with the highlight, the crate link, `#rustlang #iced`.

**Normal (Mastodon)** - at most 500 characters (every link counts as 23). One line on what
the project is, the notable changes as `•` bullets, crate and demo links, hashtags
`#Rust #RustLang #iced #Markdown #OpenSource`. Mention that it also fits Bluesky only if it is
at most 300 characters; otherwise say the bullets have to go for Bluesky.

**Ko-fi** - a title line (`Title: ...`) and a friendly post of a few short paragraphs in the
first person: what the project is, what is new and why it matters, smaller changes, links to
the demo, crate and docs, and a closing thank-you to supporters. No hashtags, no Markdown
(Ko-fi shows it as plain text).

Tone: plain and friendly, no hype words ("revolutionary", "game-changing"). At most one or two
emoji per post.

## 4. Check the length

Count the short and normal versions before showing them, e.g.:

```bash
python -c "import re,sys; t=sys.stdin.read().rstrip(); print(len(re.sub(r'https?://\S+','x'*23,t)))" <<'EOF'
...post...
EOF
```

Emoji count as 2 on X; add that by hand. Shorten until they fit.

## 5. Output

Show each version in its own `text` code block, headed with the platform and its count (e.g.
"X - 198 / 280"). Do not write files, commit, or post anything.
