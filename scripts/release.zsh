#!/usr/bin/env zsh
#
# Tag-based release helper for iced-markdown-editor, driven by CHANGELOG.md (Keep a Changelog + Semantic
# Versioning) and Cargo.toml. zsh port of release.ps1.
#
# Run from anywhere inside the repo. The tag message and the GitHub Release body both come from
# CHANGELOG.md - the "changelog" skill/tool is responsible for writing the actual entries into
# [Unreleased]; this script only promotes that section to a dated version, keeps Cargo.toml's
# version in sync (the GitHub Actions release workflow aborts the build on a mismatch), commits,
# tags, and pushes.
#
# Commands
#   list       Show the latest tags
#   rc         Test build: tags vX.Y.Z-rc.N with the notes from [Unreleased]. Nothing is modified
#              or committed - see the warning it prints about the version-check workflow.
#   new        Release: renames [Unreleased] to [X.Y.Z] - <date>, syncs Cargo.toml's version,
#              commits CHANGELOG.md/Cargo.toml/Cargo.lock, tags vX.Y.Z, pushes.
#   rollback   Delete a tag locally and on the remote, and try to remove its GitHub Release
#              (default: highest tag)
#   retag      Move a tag to HEAD and force-push it (rebuild the same version)
#   show       Show the message of a tag
#   init       Create a CHANGELOG.md skeleton
#
# Version selection (new / rc)
#   1. An explicit version: X.Y.Z
#   2. A [X.Y.Z] section in CHANGELOG.md that is newer than the latest release tag (written by
#      hand or left over from a rollback)
#   3. Otherwise Cargo.toml's current version is bumped:
#        patch   1.3.0 -> 1.3.1    (argument "patch" or --bump patch)
#        minor   1.3.0 -> 1.4.0    (argument "minor" or --bump minor)
#        major   1.3.0 -> 2.0.0    (argument "major" or --bump major - only when given explicitly)
#      Without an argument (auto) the script looks at [Unreleased] and never changes the major
#      number:
#        "### Added", "### Changed", "### Deprecated", "### Removed" or "BREAKING" -> minor
#        anything else (Fixed, Security, empty)                                   -> patch
#
# Examples
#   ./scripts/release.zsh rc                # v1.3.0-rc.1, next call v1.3.0-rc.2
#   ./scripts/release.zsh new               # v1.3.0 with notes from CHANGELOG.md
#   ./scripts/release.zsh new patch         # 1.3.0 -> 1.3.1
#   ./scripts/release.zsh new minor         # 1.3.0 -> 1.4.0
#   ./scripts/release.zsh new major         # 1.3.0 -> 2.0.0
#   ./scripts/release.zsh rollback v1.3.0-rc.2
#   ./scripts/release.zsh retag
#   ./scripts/release.zsh -h                # overview
#   ./scripts/release.zsh new -h            # details for one command

emulate -L zsh
setopt EXTENDED_GLOB PIPE_FAIL

SELF=$0

# ============================================================== output helpers

c_cyan=$'\e[36m'; c_green=$'\e[32m'; c_yellow=$'\e[33m'; c_red=$'\e[31m'; c_reset=$'\e[0m'

color() { local c=$1; shift; print -r -- "${c}$*${c_reset}"; }
warn()  { print -u2 -r -- "${c_yellow}Warning: $*${c_reset}"; }
die()   { print -u2 -r -- "${c_red}Error: $*${c_reset}"; exit 1; }

confirm() {
  (( YES )) && return 0
  local ans
  read -r "ans?$1 [y/N] "
  ans=${ans:l}
  [[ $ans == y || $ans == yes || $ans == j || $ans == ja ]]
}

# ============================================================== help

show_help() {
  local topic=$1
  case $topic in
    list)
      cat <<EOF

list
  Syncs tags with the remote and shows the latest 15 tags with date and subject.

EOF
      ;;
    rc)
      cat <<EOF

rc [patch|minor|major|X.Y.Z] [-y|--yes]
  Creates the next release candidate tag vX.Y.Z-rc.N and pushes it.
  - Version is chosen like for 'new'; N counts up per version (rc.1, rc.2, ...).
  - Tag message = [Unreleased] section (or [X.Y.Z] if it exists). Empty is allowed.
  - $CHANGELOG_FILE and $CARGO_TOML_FILE are NOT modified, nothing is committed.
  - The GitHub Actions release workflow aborts the build unless the pushed tag's version
    matches $CARGO_TOML_FILE's version and $CHANGELOG_FILE's top section exactly (rc suffix
    included). Since this command changes neither file, the pipeline will usually fail its
    version check - this is only useful to sanity-check the tag/push mechanics, not to run a
    real test build. Use 'new' for a build that actually passes CI.

EOF
      ;;
    new)
      cat <<EOF

new [patch|minor|major|X.Y.Z] [-y|--yes]
  Creates a real release.
  1. Picks the version:
       - X.Y.Z if given
       - else an untagged [X.Y.Z] section in the changelog (hand-written or after a rollback)
       - else bumps Cargo.toml's current version:
           patch -> 1.3.0 -> 1.3.1
           minor -> 1.3.0 -> 1.4.0
           major -> 1.3.0 -> 2.0.0  (only when given explicitly)
           none  -> auto from [Unreleased], never major:
                    ### Added / Changed / Deprecated / Removed or BREAKING  -> minor
                    anything else                                           -> patch
  2. If notes come from [Unreleased]: renames it to [X.Y.Z] - <today>, adds a new empty
     [Unreleased], and updates the compare links at the bottom of the changelog.
  3. If Cargo.toml's version differs from X.Y.Z, updates it (and runs 'cargo check' to refresh
     Cargo.lock).
  4. Re-checks that Cargo.toml's version and the changelog's top section agree - the same
     check the release workflow runs - before committing anything.
  5. Commits whatever changed above as "Release vX.Y.Z" and pushes the branch.
  6. Creates annotated tag vX.Y.Z with the section as message and pushes it.
  Fails if the section has no entries, the tag exists, or the branch is behind the remote.

EOF
      ;;
    rollback)
      cat <<EOF

rollback [tag] [-y|--yes]
  Deletes the tag on the remote and locally (default: highest tag, rc tags included).
  If the GitHub CLI ('gh') is installed and a GitHub Release exists for that tag, offers to
  delete it too. Without 'gh', delete the release manually if one was published. Built
  artifacts already downloaded by others stay untouched.
  $CHANGELOG_FILE and $CARGO_TOML_FILE are not touched: running 'new' again re-releases the
  same version.

EOF
      ;;
    retag)
      cat <<EOF

retag [tag] [-y|--yes]
  Moves an existing tag (default: highest) to the current HEAD, keeps its message,
  and force-pushes it so the pipeline builds the same version again.
  Note: if a GitHub Release already exists for that tag, re-running the workflow may fail to
  publish over it depending on the release action's settings - check the run if that happens.

EOF
      ;;
    show)
      cat <<EOF

show [tag]
  Prints the tag message and the commit it points to (default: highest tag).

EOF
      ;;
    init)
      cat <<EOF

init [--changelog <path>]
  Creates a CHANGELOG.md skeleton with an [Unreleased] section. Fails if it already exists.

EOF
      ;;
    *)
      cat <<EOF

Release helper for iced-markdown-editor - binaries are built by GitHub Actions, release notes come from
$CHANGELOG_FILE (Keep a Changelog + Semantic Versioning). This script does not write changelog
*content*; use the changelog skill/tool for that. It only promotes [Unreleased] to a dated
version section, keeps $CARGO_TOML_FILE's version in sync, commits, tags, and pushes.

USAGE
  $SELF <command> [version] [options]
  $SELF <command> -h           detailed help for one command

COMMANDS
  list                 Show the latest 15 tags (default)
  rc   [version]       Test build: tag vX.Y.Z-rc.N, notes from [Unreleased], nothing committed
  new  [version]       Release: [Unreleased] -> [X.Y.Z], sync Cargo.toml, commit, tag vX.Y.Z, push
  rollback [tag]       Delete a tag locally + on the remote, try to remove its GitHub Release
                       (default: highest tag)
  retag    [tag]       Move a tag to HEAD and force-push it -> rebuild same version
  show     [tag]       Show message and commit of a tag (default: highest tag)
  init                 Create a CHANGELOG.md skeleton
  help                 Show this help

VERSION (for rc / new)
  (none)     auto: patch or minor, depending on [Unreleased] - never major
  patch      1.3.0 -> 1.3.1
  minor      1.3.0 -> 1.4.0
  major      1.3.0 -> 2.0.0
  X.Y.Z      exactly this version

OPTIONS
  --bump auto|patch|minor|major   Same as the version argument (default: auto)
  --changelog <path>              Changelog file, relative to repo root (default: CHANGELOG.md)
  --cargo-toml <path>             Cargo manifest, relative to repo root (default: Cargo.toml)
  --remote <name>                 Git remote (default: origin)
  -y, --yes                       Skip confirmation prompts
  -h, --help                      Show help

EXAMPLES (latest release v1.2.3)
  $SELF rc                   test build  -> v1.3.0-rc.1
  $SELF new                  release     -> v1.3.0
  $SELF new patch            release     -> v1.2.4   (only last number)
  $SELF new minor            release     -> v1.3.0   (second last number)
  $SELF rc minor             test build  -> v1.3.0-rc.1
  $SELF rollback v1.3.0-rc.1
  $SELF retag -y

EOF
      ;;
  esac
}

# ============================================================== git helpers

assert_ready() {
  local rel=$CHANGELOG_FILE
  local dirty=() line
  while IFS= read -r line; do
    [[ -z $line ]] && continue
    [[ ${line:3} == $rel ]] && continue
    dirty+=("$line")
  done < <(git status --porcelain)
  if (( ${#dirty} )); then
    warn "Uncommitted changes - they will NOT be part of the build:
${(F)dirty}"
  fi
  local ahead
  ahead=$(git rev-list --count '@{u}..HEAD' 2>/dev/null) || ahead=""
  if [[ -n $ahead ]] && (( ahead > 0 )); then
    warn "$ahead commit(s) not pushed to your branch yet. The tag still includes them."
  fi
}

assert_can_commit() {
  local branch
  branch=$(git symbolic-ref --quiet --short HEAD 2>/dev/null) || die 'Detached HEAD - check out a branch to create a release.'
  local behind
  behind=$(git rev-list --count 'HEAD..@{u}' 2>/dev/null) || behind=""
  if [[ -n $behind ]] && (( behind > 0 )); then
    die "Branch $branch is $behind commit(s) behind $REMOTE. Pull first."
  fi
  print -r -- "$branch"
}

repo_web_url() {
  local url
  url=$(git remote get-url "$REMOTE" 2>/dev/null) || return 1
  url=${url%$'\r'}
  [[ -z $url ]] && return 1

  if [[ $url == git@*:* ]]; then
    local host=${url#git@}; host=${host%%:*}
    local path=${url#*:}; path=${path%.git}
    print -r -- "https://$host/$path"
    return 0
  fi
  if [[ $url == ssh://git@* ]]; then
    local rest=${url#ssh://git@}
    local host=${rest%%/*}; host=${host%%:*}
    local path=${rest#*/}; path=${path%.git}
    print -r -- "https://$host/$path"
    return 0
  fi
  if [[ $url == http://* || $url == https://* ]]; then
    local rest=${url#*://}
    rest=${rest#*@}
    rest=${rest%.git}
    print -r -- "https://$rest"
    return 0
  fi
  return 1
}

# --cleanup=whitespace keeps markdown headings like "### Added" (default mode strips lines starting with #)
make_annotated_tag() {
  local name=$1 text=$2 target=${3:-HEAD} force=${4:-0}
  local tmpfile
  tmpfile=$(mktemp)
  printf '%s' "$text" > "$tmpfile"
  local args=(tag -a --cleanup=whitespace -F "$tmpfile")
  (( force )) && args+=(-f)
  args+=("$name" "$target")
  git "${args[@]}"
  local rc=$?
  rm -f "$tmpfile"
  (( rc != 0 )) && die "git tag failed (exit code $rc)"
}

remove_github_release() {
  local tag=$1
  if ! command -v gh >/dev/null 2>&1; then
    local web where
    web=$(repo_web_url) || web=""
    if [[ -n $web ]]; then where="$web/releases/tag/$tag"; else where="GitHub"; fi
    warn "GitHub CLI ('gh') not found - if a GitHub Release was published for $tag, delete it manually at $where."
    return 0
  fi
  gh release view "$tag" >/dev/null 2>&1 || return 0
  confirm "A GitHub Release exists for $tag. Delete it too?" || return 0
  if ! gh release delete "$tag" --yes; then
    warn "Failed to delete the GitHub Release for $tag - remove it manually if needed."
  fi
}

# ============================================================== versions

# Parses X.Y.Z or vX.Y.Z(-rc.N) into VER_* globals. Returns 1 on no match.
ver_parse() {
  local text=$1
  if [[ $text =~ '^v?([0-9]+)\.([0-9]+)\.([0-9]+)(-rc\.([0-9]+))?$' ]]; then
    VER_MAJOR=${match[1]}; VER_MINOR=${match[2]}; VER_PATCH=${match[3]}
    if [[ -n ${match[4]-} ]]; then
      VER_IS_RC=1; VER_RC=${match[5]}
    else
      VER_IS_RC=0; VER_RC=0
    fi
    VER_CORE="$VER_MAJOR.$VER_MINOR.$VER_PATCH"
    VER_TAG="v$VER_CORE"
    (( VER_IS_RC )) && VER_TAG+="-rc.$VER_RC"
    # rc versions sort before the final release of the same version
    VER_SORTKEY=$(printf '%06d.%06d.%06d.%d.%06d' "$VER_MAJOR" "$VER_MINOR" "$VER_PATCH" $(( VER_IS_RC ? 0 : 1 )) "$VER_RC")
    return 0
  fi
  return 1
}

# Prints "sortkey|tag|core|major|minor|patch|isrc|rc" lines for all v* tags, sorted by sortkey.
version_tags() {
  git tag --list 'v*' | while IFS= read -r t; do
    if ver_parse "$t"; then
      printf '%s|%s|%s|%d|%d|%d|%d|%d\n' "$VER_SORTKEY" "$VER_TAG" "$VER_CORE" "$VER_MAJOR" "$VER_MINOR" "$VER_PATCH" "$VER_IS_RC" "$VER_RC"
    fi
  done | sort
}

# Sets LATEST_* globals from the highest tag. $1=1 to consider final releases only. Returns 1 if none.
latest_tag() {
  local final_only=$1 line
  if (( final_only )); then
    line=$(version_tags | awk -F'|' '$7==0' | tail -n1)
  else
    line=$(version_tags | tail -n1)
  fi
  [[ -z $line ]] && return 1
  IFS='|' read -r LATEST_SORTKEY LATEST_TAG LATEST_CORE LATEST_MAJOR LATEST_MINOR LATEST_PATCH LATEST_IS_RC LATEST_RC <<< "$line"
  return 0
}

resolve_tag_name() {
  local name=$1
  if [[ -z $name ]]; then
    latest_tag 0 || die "No version tags found."
    print -r -- "$LATEST_TAG"
    return 0
  fi
  [[ $name == v* ]] || name="v$name"
  git tag --list "$name" | grep -qx -- "$name" || die "Tag $name does not exist on $REMOTE."
  print -r -- "$name"
}

step_version() {
  local major=$1 minor=$2 patch=$3 level=$4
  case $level in
    major) (( major++ )); minor=0; patch=0 ;;
    minor) (( minor++ )); patch=0 ;;
    patch) (( patch++ )) ;;
  esac
  print -r -- "$major.$minor.$patch"
}

# ============================================================== line-buffer helpers (Cargo.toml / CHANGELOG.md)

# Reads a file into the global RESULT_LINES array (CR-stripped). $2=optional hint for the error.
read_lines_into_result() {
  local path=$1 hint=${2:-}
  if [[ ! -f $path ]]; then
    if [[ -n $hint ]]; then die "$path not found. $hint"; else die "$path not found."; fi
  fi
  RESULT_LINES=()
  local line
  while IFS= read -r line || [[ -n $line ]]; do
    RESULT_LINES+=("${line%$'\r'}")
  done < "$path"
}

# ---- Cargo.toml (operates on the global CARGO_LINES array) ----

cargo_version_get() {
  local l
  for l in "${CARGO_LINES[@]}"; do
    if [[ $l =~ '^version[[:space:]]*=[[:space:]]*"([^"]+)"' ]]; then
      print -r -- "${match[1]}"
      return 0
    fi
  done
  return 1
}

cargo_version_set() {
  local newver=$1 i n=${#CARGO_LINES}
  for (( i = 1; i <= n; i++ )); do
    if [[ ${CARGO_LINES[i]} =~ '^version[[:space:]]*=[[:space:]]*"([^"]+)"' ]]; then
      CARGO_LINES[i]="version = \"$newver\""
      return 0
    fi
  done
  return 1
}

# ---- CHANGELOG.md (section-finding operates on the global CHANGELOG_LINES array) ----

# Finds "## [Name]" (v-prefix ignored). Sets SECTION_START/SECTION_END (End = index *after* the body).
find_section() {
  local name=$1 i n=${#CHANGELOG_LINES}
  for (( i = 1; i <= n; i++ )); do
    if [[ ${CHANGELOG_LINES[i]} =~ '^##[[:space:]]+\[([^]]+)\]' ]]; then
      local heading=${match[1]#v}
      if [[ $heading == $name ]]; then
        local end=$(( i + 1 ))
        while (( end <= n )) \
          && [[ ! ${CHANGELOG_LINES[end]} =~ '^##[[:space:]]' ]] \
          && [[ ! ${CHANGELOG_LINES[end]} =~ '^\[[^]]+\]:[[:space:]]' ]]; do
          (( end++ ))
        done
        SECTION_START=$i; SECTION_END=$end
        return 0
      fi
    fi
  done
  return 1
}

# $1=start $2=end; trims leading/trailing blank lines; sets SECTION_BODY
get_section_body() {
  local start=$1 end=$2
  SECTION_BODY=()
  local first=$(( start + 1 )) last=$(( end - 1 ))
  (( first > last )) && return 0
  local tmp=("${(@)CHANGELOG_LINES[first,last]}")
  local a=1 b=${#tmp}
  while (( a <= b )) && [[ -z ${tmp[a]//[[:space:]]/} ]]; do (( a++ )); done
  while (( b >= a )) && [[ -z ${tmp[b]//[[:space:]]/} ]]; do (( b-- )); done
  (( a > b )) && return 0
  SECTION_BODY=("${(@)tmp[a,b]}")
}

# Drops "### Xyz" headings with no content below them. Reads WORK_BODY, sets CLEAN_BODY.
remove_empty_subsections() {
  CLEAN_BODY=()
  local pending=() have_pending=0 line
  for line in "${WORK_BODY[@]}"; do
    if [[ $line =~ '^###[[:space:]]' ]]; then
      pending=("$line"); have_pending=1
      continue
    fi
    if (( have_pending )); then
      if [[ -z ${line//[[:space:]]/} ]]; then pending+=("$line"); continue; fi
      CLEAN_BODY+=("${pending[@]}")
      pending=(); have_pending=0
    fi
    CLEAN_BODY+=("$line")
  done
  local last=${#CLEAN_BODY}
  while (( last > 0 )) && [[ -z ${CLEAN_BODY[last]//[[:space:]]/} ]]; do (( last-- )); done
  if (( last >= 1 )); then CLEAN_BODY=("${(@)CLEAN_BODY[1,last]}"); else CLEAN_BODY=(); fi
}

has_entries() {  # reads WORK_BODY
  local l
  for l in "${WORK_BODY[@]}"; do
    [[ $l =~ '^[[:space:]]*[-*][[:space:]]+[^[:space:]]' ]] && return 0
  done
  return 1
}

auto_bump() {  # reads UNRELEASED_BODY; prints patch|minor - never major
  local -A types
  local current='' line
  for line in "${UNRELEASED_BODY[@]}"; do
    if [[ $line =~ '^###[[:space:]]+(.+)[[:space:]]*$' ]]; then
      current=${match[1]:l}
      continue
    fi
    if [[ $line =~ '^[[:space:]]*[-*][[:space:]]+[^[:space:]]' ]]; then
      types[$current]=$(( ${types[$current]:-0} + 1 ))
    fi
  done
  local level=patch
  if [[ -n ${types[added]-} || -n ${types[changed]-} || -n ${types[deprecated]-} || -n ${types[removed]-} ]]; then
    level=minor
  fi
  local joined
  joined=$(printf '%s\n' "${UNRELEASED_BODY[@]}")
  if [[ -n ${types[removed]-} || $joined == *BREAKING* ]]; then
    level=minor
    warn "[Unreleased] contains breaking changes or removals. Auto bump stays at minor - use 'major' if you want a new major version."
  fi
  print -r -- "$level"
}

# Resolves the version to release. Sets TARGET_CORE, TARGET_REASON, TARGET_SOURCE,
# TARGET_HAS_OWN_SECTION (0/1), TARGET_BODY (array), TARGET_LATEST_FINAL_TAG (may be empty),
# and UNRELEASED_BODY (array, also used by callers to warn about leftover entries).
resolve_target() {
  local current_cargo_version=$1

  local have_latest_final=0 latest_final_tag='' latest_final_sortkey=''
  if latest_tag 1; then
    have_latest_final=1
    latest_final_tag=$LATEST_TAG
    latest_final_sortkey=$LATEST_SORTKEY
  fi

  ver_parse "$current_cargo_version" || die "$CARGO_TOML_FILE has an unexpected version '$current_cargo_version' (expected X.Y.Z)."
  local cur_major=$VER_MAJOR cur_minor=$VER_MINOR cur_patch=$VER_PATCH

  UNRELEASED_BODY=()
  if find_section 'Unreleased'; then
    get_section_body "$SECTION_START" "$SECTION_END"
    UNRELEASED_BODY=("${SECTION_BODY[@]}")
  fi

  local core='' reason=''

  if [[ -n $VERSION ]]; then
    core=${VERSION#v}
    core=${core%-rc.[0-9]##}
    ver_parse "$core" || die "Invalid version '$VERSION' (expected X.Y.Z)."
    reason='given explicitly'
  else
    local top_core='' top_sortkey=''
    local i n=${#CHANGELOG_LINES}
    for (( i = 1; i <= n; i++ )); do
      if [[ ${CHANGELOG_LINES[i]} =~ '^##[[:space:]]+\[v?([0-9]+\.[0-9]+\.[0-9]+)\]' ]]; then
        ver_parse "${match[1]}" && { top_core=$VER_CORE; top_sortkey=$VER_SORTKEY; }
        break
      fi
    done
    if [[ -n $top_core ]] && { (( ! have_latest_final )) || [[ $top_sortkey > $latest_final_sortkey ]]; }; then
      core=$top_core
      reason="untagged section [$core] in $CHANGELOG_FILE"
      [[ $BUMP != auto ]] && warn "'$BUMP' ignored: $CHANGELOG_FILE already contains the untagged section [$core]. Pass an explicit X.Y.Z to override."
    else
      local level=$BUMP
      [[ $BUMP == auto ]] && level=$(auto_bump)
      core=$(step_version "$cur_major" "$cur_minor" "$cur_patch" "$level")
      local how='requested'
      [[ $BUMP == auto ]] && how='auto from [Unreleased]'
      reason="$level bump ($how) from $CARGO_TOML_FILE $current_cargo_version"
    fi
  fi

  TARGET_BODY=(); TARGET_SOURCE='[Unreleased]'; TARGET_HAS_OWN_SECTION=0
  if find_section "$core"; then
    get_section_body "$SECTION_START" "$SECTION_END"
    TARGET_BODY=("${SECTION_BODY[@]}")
    TARGET_SOURCE="[$core]"
    TARGET_HAS_OWN_SECTION=1
  else
    TARGET_BODY=("${UNRELEASED_BODY[@]}")
  fi

  TARGET_CORE=$core
  TARGET_REASON=$reason
  TARGET_LATEST_FINAL_TAG=""
  (( have_latest_final )) && TARGET_LATEST_FINAL_TAG=$latest_final_tag
}

# [Unreleased] -> [X.Y.Z] - date, plus a fresh empty [Unreleased] on top, plus compare links.
update_changelog_for_release() {
  local core=$1 prevtag=$2
  local tag="v$core"

  find_section 'Unreleased' || die "No [Unreleased] section found in $CHANGELOG_FILE."
  local ustart=$SECTION_START uend=$SECTION_END
  get_section_body "$ustart" "$uend"
  WORK_BODY=("${SECTION_BODY[@]}")
  remove_empty_subsections
  local clean=("${CLEAN_BODY[@]}")

  local today
  today=$(date +%Y-%m-%d)

  local before=() after=()
  (( ustart > 1 )) && before=("${(@)CHANGELOG_LINES[1,ustart-1]}")
  (( uend <= ${#CHANGELOG_LINES} )) && after=("${(@)CHANGELOG_LINES[uend,${#CHANGELOG_LINES}]}")

  CHANGELOG_LINES=(
    "${before[@]}"
    '## [Unreleased]'
    ''
    "## [$core] - $today"
    ''
    "${clean[@]}"
    ''
    "${after[@]}"
  )

  local web
  web=$(repo_web_url) || return 0

  local unreleased_link="[unreleased]: $web/compare/$tag...HEAD"
  local version_link
  if [[ -n $prevtag ]]; then
    version_link="[$core]: $web/compare/$prevtag...$tag"
  else
    version_link="[$core]: $web/releases/tag/$tag"
  fi

  local i n=${#CHANGELOG_LINES} link_index=0
  for (( i = 1; i <= n; i++ )); do
    if [[ ${CHANGELOG_LINES[i]} =~ '^\[unreleased\]:[[:space:]]' ]]; then
      link_index=$i
      break
    fi
  done

  if (( link_index > 0 )); then
    CHANGELOG_LINES[link_index]="$unreleased_link"
    CHANGELOG_LINES=("${(@)CHANGELOG_LINES[1,link_index]}" "$version_link" "${(@)CHANGELOG_LINES[link_index+1,${#CHANGELOG_LINES}]}")
  else
    local insert_at=${#CHANGELOG_LINES}
    while (( insert_at > 0 )) && [[ -z ${CHANGELOG_LINES[insert_at]//[[:space:]]/} ]]; do (( insert_at-- )); done
    CHANGELOG_LINES=("${(@)CHANGELOG_LINES[1,insert_at]}" '' "$unreleased_link" "$version_link" "${(@)CHANGELOG_LINES[insert_at+1,${#CHANGELOG_LINES}]}")
  fi
}

format_tag_message() {  # $1=title; reads WORK_BODY; prints "title\n\nbody\n"
  local title=$1
  local text='(no changelog entries)'
  if has_entries; then
    remove_empty_subsections
    text=$(printf '%s\n' "${CLEAN_BODY[@]}")
  fi
  printf '%s\n\n%s\n' "$title" "$text"
}

# Mirrors the version-match check in .github/workflows/release.yml, reading both files fresh from disk.
assert_version_consistency() {
  local cargo_ver
  cargo_ver=$(grep -m1 -E '^version[[:space:]]*=[[:space:]]*"' "$CARGO_TOML_PATH" | sed -E 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/')
  [[ -n $cargo_ver ]] || die "No top-level 'version' field found in $CARGO_TOML_PATH."

  local changelog_line
  changelog_line=$(grep -m1 -E '^##[[:space:]]+\[[0-9]' "$CHANGELOG_PATH") || true
  local changelog_ver=''
  if [[ -n $changelog_line && $changelog_line =~ '\[([0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?)\]' ]]; then
    changelog_ver=${match[1]}
  fi
  [[ -n $changelog_ver ]] || die "Could not find a versioned '## [X.Y.Z]' section at the top of $CHANGELOG_PATH."

  if [[ $cargo_ver != $changelog_ver ]]; then
    die "Version mismatch: $CARGO_TOML_PATH has '$cargo_ver' but $CHANGELOG_PATH's latest section is [$changelog_ver]. The GitHub Actions build would abort on this - fix it before tagging."
  fi
}

# ============================================================== commands

cmd_list() {
  local out
  out=$(git for-each-ref refs/tags --sort=-v:refname --count=15 --format='%(refname:short)|%(creatordate:short)|%(contents:subject)')
  if [[ -z $out ]]; then
    print -r -- "No tags on $REMOTE yet."
    return
  fi
  {
    printf 'TAG\tDATE\tSUBJECT\n'
    local tg dt subj
    while IFS='|' read -r tg dt subj; do
      printf '%s\t%s\t%s\n' "$tg" "$dt" "$subj"
    done <<< "$out"
  } | column -t -s $'\t'
}

cmd_rc() {
  assert_ready
  read_lines_into_result "$CHANGELOG_PATH" "Run '$SELF init' to create one."
  CHANGELOG_LINES=("${RESULT_LINES[@]}")
  read_lines_into_result "$CARGO_TOML_PATH"
  CARGO_LINES=("${RESULT_LINES[@]}")
  local current_cargo_version
  current_cargo_version=$(cargo_version_get) || die "No top-level 'version' field found in $CARGO_TOML_PATH."

  resolve_target "$current_cargo_version"

  git tag --list "v$TARGET_CORE" | grep -qx "v$TARGET_CORE" && die "v$TARGET_CORE is already released. Use an explicit version for another version."

  local max_rc=0 sk tg co maj mn pa isrc rc
  while IFS='|' read -r sk tg co maj mn pa isrc rc; do
    [[ $co == $TARGET_CORE && $isrc == 1 ]] || continue
    (( rc > max_rc )) && max_rc=$rc
  done < <(version_tags)
  local n=$(( max_rc + 1 ))
  local tag="v${TARGET_CORE}-rc.${n}"
  WORK_BODY=("${TARGET_BODY[@]}")
  local message
  message=$(format_tag_message "Release candidate $tag")

  print -r -- ""
  color "$c_cyan" "Tag:     $tag"
  print -r -- "Version: $TARGET_REASON"
  print -r -- "Commit:  $(git log -1 --oneline)"
  print -r -- "Notes:   $TARGET_SOURCE from $CHANGELOG_FILE ($CHANGELOG_FILE and $CARGO_TOML_FILE are not modified)"
  print -r -- ""
  print -r -- "$message"

  if [[ $current_cargo_version != $TARGET_CORE ]]; then
    warn "$CARGO_TOML_FILE is at $current_cargo_version, not $TARGET_CORE. The release workflow's version check compares the tag ($tag) against $CARGO_TOML_FILE and $CHANGELOG_FILE exactly, so this build will likely fail validation. Use 'new' for a build that actually passes CI."
  fi

  confirm "Create and push $tag?" || { print -r -- "Aborted."; return; }
  make_annotated_tag "$tag" "$message"
  git push "$REMOTE" "refs/tags/$tag" || die "git push failed"
  color "$c_green" "Pushed $tag - GitHub Actions should start."
}

cmd_new() {
  assert_ready
  read_lines_into_result "$CHANGELOG_PATH" "Run '$SELF init' to create one."
  CHANGELOG_LINES=("${RESULT_LINES[@]}")
  read_lines_into_result "$CARGO_TOML_PATH"
  CARGO_LINES=("${RESULT_LINES[@]}")
  local current_cargo_version
  current_cargo_version=$(cargo_version_get) || die "No top-level 'version' field found in $CARGO_TOML_PATH."

  resolve_target "$current_cargo_version"
  local tag="v$TARGET_CORE"

  git tag --list "$tag" | grep -qx "$tag" && die "Tag $tag already exists. Use 'retag $tag' to rebuild or 'rollback $tag' first."
  WORK_BODY=("${TARGET_BODY[@]}")
  has_entries || die "No entries in $TARGET_SOURCE of $CHANGELOG_FILE. Describe your changes there first (or use 'rc' for a test build)."

  local promote_changelog=0 bump_cargo=0
  (( TARGET_HAS_OWN_SECTION )) || promote_changelog=1
  [[ $current_cargo_version != $TARGET_CORE ]] && bump_cargo=1
  local needs_commit=0
  (( promote_changelog || bump_cargo )) && needs_commit=1

  local branch=""
  WORK_BODY=("${UNRELEASED_BODY[@]}")
  if (( needs_commit )); then
    branch=$(assert_can_commit)
  elif has_entries; then
    warn "[Unreleased] also has entries - they stay there for the next release."
  fi

  local prev_tag=$TARGET_LATEST_FINAL_TAG
  local prev_display="-"
  [[ -n $prev_tag ]] && prev_display=$prev_tag
  WORK_BODY=("${TARGET_BODY[@]}")
  local message
  message=$(format_tag_message "Release $tag")

  print -r -- ""
  color "$c_cyan" "Tag:       $tag  (previous release: $prev_display)"
  print -r -- "Version:   $TARGET_REASON"
  print -r -- "Commit:    $(git log -1 --oneline)"
  print -r -- "Notes:     $TARGET_SOURCE from $CHANGELOG_FILE"
  (( bump_cargo )) && print -r -- "$CARGO_TOML_FILE: $current_cargo_version -> $TARGET_CORE"
  (( promote_changelog )) && print -r -- "Changelog: [Unreleased] -> [$TARGET_CORE] - $(date +%Y-%m-%d)"
  (( needs_commit )) && print -r -- "Commit:    'Release $tag' pushed to $branch"
  print -r -- ""
  print -r -- "$message"

  confirm "Release $tag?" || { print -r -- "Aborted."; return; }

  local changed_paths=()

  if (( promote_changelog )); then
    update_changelog_for_release "$TARGET_CORE" "$prev_tag"
    printf '%s\n' "${CHANGELOG_LINES[@]}" > "$CHANGELOG_PATH"
    changed_paths+=("$CHANGELOG_PATH")
  fi

  if (( bump_cargo )); then
    cargo_version_set "$TARGET_CORE"
    printf '%s\n' "${CARGO_LINES[@]}" > "$CARGO_TOML_PATH"
    changed_paths+=("$CARGO_TOML_PATH")

    if command -v cargo >/dev/null 2>&1; then
      print -r -- "Updating Cargo.lock (cargo check)..."
      cargo check --quiet || die "cargo check failed while refreshing Cargo.lock."
      [[ -f $CARGO_LOCK_PATH ]] && changed_paths+=("$CARGO_LOCK_PATH")
    else
      warn "cargo not found on PATH - Cargo.lock was not refreshed. Run 'cargo check' before pushing if Cargo.lock tracks the package version."
    fi
  fi

  assert_version_consistency

  if (( needs_commit )); then
    git add -- "${changed_paths[@]}"
    git commit --quiet -m "Release $tag" || die "git commit failed"
    git push --quiet "$REMOTE" "HEAD:refs/heads/$branch" || die "git push failed"
  fi

  make_annotated_tag "$tag" "$message"
  git push "$REMOTE" "refs/tags/$tag" || die "git push failed"
  color "$c_green" "Released $tag - GitHub Actions should start the build."
}

cmd_rollback() {
  local tag
  tag=$(resolve_tag_name "$VERSION")
  print -r -- ""
  git --no-pager show --no-patch "$tag"
  print -r -- ""
  warn 'Only the tag is removed here. A GitHub Release attached to it stays online unless removed separately (see below). Built artifacts already downloaded by others are untouched.'

  confirm "Delete $tag locally and on ${REMOTE}?" || { print -r -- "Aborted."; return; }
  git push "$REMOTE" --delete "refs/tags/$tag" || die "git push --delete failed"
  git tag -d "$tag" || die "git tag -d failed"
  remove_github_release "$tag"

  local prev="none"
  latest_tag 0 && prev=$LATEST_TAG
  color "$c_green" "Deleted $tag. Highest tag is now: $prev"
  if [[ $tag != *-rc.* ]]; then
    print -r -- "The [${tag#v}] section stays in $CHANGELOG_FILE and the version stays in $CARGO_TOML_FILE - running 'new' again re-releases it."
  fi
}

cmd_retag() {
  assert_ready
  local tag
  tag=$(resolve_tag_name "$VERSION")
  local message
  message=$(git tag --list --format='%(contents)' "$tag")
  message="${message%$'\n'}"$'\n'

  print -r -- ""
  color "$c_cyan" "Tag:  $tag"
  print -r -- "From: $(git log -1 --oneline "${tag}^{commit}")"
  print -r -- "To:   $(git log -1 --oneline HEAD)"
  print -r -- ""

  confirm "Move $tag to HEAD and force-push? (triggers a new GitHub Actions run)" || { print -r -- "Aborted."; return; }
  make_annotated_tag "$tag" "$message" HEAD 1
  git push --force "$REMOTE" "refs/tags/$tag" || die "git push failed"
  color "$c_green" "Re-pushed $tag."
}

cmd_show() {
  local tag
  tag=$(resolve_tag_name "$VERSION")
  git --no-pager show --no-patch "$tag"
}

cmd_init() {
  [[ -f $CHANGELOG_PATH ]] && die "$CHANGELOG_PATH already exists."
  cat > "$CHANGELOG_PATH" <<'EOF'
# Changelog

All notable changes to this project are documented in this file.
Format: Keep a Changelog (https://keepachangelog.com), versioning: Semantic Versioning (https://semver.org).

## [Unreleased]

### Added

### Changed

### Fixed

EOF
  color "$c_green" "Created $CHANGELOG_PATH - commit it together with scripts/release.zsh."
}

# ============================================================== main

typeset -ga CHANGELOG_LINES=() CARGO_LINES=() RESULT_LINES=() SECTION_BODY=() WORK_BODY=() CLEAN_BODY=() UNRELEASED_BODY=() TARGET_BODY=()

COMMAND=""
VERSION=""
BUMP=auto
CHANGELOG_FILE=CHANGELOG.md
CARGO_TOML_FILE=Cargo.toml
REMOTE=origin
YES=0
HELP=0

positional=()
while (( $# )); do
  case $1 in
    -h|--help) HELP=1; shift ;;
    -y|--yes) YES=1; shift ;;
    --bump) BUMP=$2; shift 2 ;;
    --bump=*) BUMP=${1#*=}; shift ;;
    --changelog) CHANGELOG_FILE=$2; shift 2 ;;
    --changelog=*) CHANGELOG_FILE=${1#*=}; shift ;;
    --cargo-toml) CARGO_TOML_FILE=$2; shift 2 ;;
    --cargo-toml=*) CARGO_TOML_FILE=${1#*=}; shift ;;
    --remote) REMOTE=$2; shift 2 ;;
    --remote=*) REMOTE=${1#*=}; shift ;;
    --) shift; while (( $# )); do positional+=("$1"); shift; done ;;
    -*) die "Unknown option: $1" ;;
    *) positional+=("$1"); shift ;;
  esac
done

case $BUMP in
  auto|patch|minor|major) ;;
  *) die "Invalid --bump value '$BUMP' (expected auto|patch|minor|major)" ;;
esac

command_given=0
if (( ${#positional} >= 1 )); then
  COMMAND=${positional[1]}
  command_given=1
else
  COMMAND=list
fi
(( ${#positional} >= 2 )) && VERSION=${positional[2]}

case $COMMAND in
  list|rc|new|rollback|retag|show|init|help) ;;
  *) die "Unknown command '$COMMAND' (expected list|rc|new|rollback|retag|show|init|help)" ;;
esac

if (( HELP )) || [[ $COMMAND == help ]]; then
  topic=""
  if (( HELP )); then
    (( command_given )) && topic=$COMMAND
  else
    [[ -n $VERSION ]] && topic=$VERSION
  fi
  show_help "$topic"
  exit 0
fi

# "new patch" / "rc minor" is shorthand for --bump
if [[ $COMMAND == (new|rc) ]] && [[ $VERSION == (patch|minor|major) ]]; then
  BUMP=$VERSION
  VERSION=""
fi

ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || die "Not inside a git repository."
cd -- "$ROOT" || die "Could not cd to $ROOT"

CHANGELOG_PATH=$CHANGELOG_FILE
CARGO_TOML_PATH=$CARGO_TOML_FILE
CARGO_LOCK_PATH=Cargo.lock

if [[ $COMMAND != init ]]; then
  # local tags = remote tags (removes stale ones, updates moved ones)
  git fetch "$REMOTE" --prune --prune-tags --force --quiet
fi

case $COMMAND in
  list) cmd_list ;;
  rc) cmd_rc ;;
  new) cmd_new ;;
  rollback) cmd_rollback ;;
  retag) cmd_retag ;;
  show) cmd_show ;;
  init) cmd_init ;;
esac
