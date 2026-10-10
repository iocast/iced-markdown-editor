<#
.SYNOPSIS
  Tag-based release helper for iced-markdown-editor, driven by CHANGELOG.md (Keep a Changelog + Semantic Versioning)
  and Cargo.toml.

.DESCRIPTION
  Run from anywhere inside the repo. The tag message and the GitHub Release body both come from
  CHANGELOG.md - the "changelog" skill is responsible for writing the actual entries into
  [Unreleased]; this script only promotes that section to a dated version, keeps Cargo.toml's
  version in sync (the GitHub Actions release workflow aborts the build on a mismatch), commits,
  tags, and pushes.

  Commands
    list       Show the latest tags
    rc         Test build: tags vX.Y.Z-rc.N with the notes from [Unreleased]. Nothing is modified
               or committed - see the warning it prints about the version-check workflow.
    new        Release: renames [Unreleased] to [X.Y.Z] - <date>, syncs Cargo.toml's version,
               commits CHANGELOG.md/Cargo.toml/Cargo.lock, tags vX.Y.Z, pushes.
    rollback   Delete a tag locally and on the remote, and try to remove its GitHub Release
               (default: highest tag)
    retag      Move a tag to HEAD and force-push it (rebuild the same version)
    show       Show the message of a tag
    init       Create a CHANGELOG.md skeleton

  Version selection (new / rc)
    1. An explicit version: X.Y.Z
    2. A [X.Y.Z] section in CHANGELOG.md that is newer than the latest release tag (written by hand
       or left over from a rollback)
    3. Otherwise Cargo.toml's current version is bumped:
         patch   1.3.0 -> 1.3.1    (argument "patch" or -Bump patch)
         minor   1.3.0 -> 1.4.0    (argument "minor" or -Bump minor)
         major   1.3.0 -> 2.0.0    (argument "major" or -Bump major - only when given explicitly)
       Without an argument (auto) the script looks at [Unreleased] and never changes the major number:
         "### Added", "### Changed", "### Deprecated", "### Removed" or "BREAKING" -> minor
         anything else (Fixed, Security, empty)                                   -> patch

.EXAMPLE
  ./scripts/release.ps1 rc                # v1.3.0-rc.1, next call v1.3.0-rc.2
  ./scripts/release.ps1 new               # v1.3.0 with notes from CHANGELOG.md
  ./scripts/release.ps1 new patch         # 1.3.0 -> 1.3.1
  ./scripts/release.ps1 new minor         # 1.3.0 -> 1.4.0
  ./scripts/release.ps1 new major         # 1.3.0 -> 2.0.0
  ./scripts/release.ps1 rollback v1.3.0-rc.2
  ./scripts/release.ps1 retag
  ./scripts/release.ps1 -h                # overview
  ./scripts/release.ps1 new -h            # details for one command
#>
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('list', 'rc', 'new', 'rollback', 'retag', 'show', 'init', 'help')]
    [string]$Command = 'list',

    [Parameter(Position = 1)]
    [string]$Version,

    [ValidateSet('auto', 'patch', 'minor', 'major')]
    [string]$Bump = 'auto',

    [string]$Changelog = 'CHANGELOG.md',

    [string]$CargoToml = 'Cargo.toml',

    [string]$Remote = 'origin',

    [switch]$Yes,

    [Alias('h')]
    [switch]$Help
)

$Self = $MyInvocation.InvocationName

# ============================================================== help

function Show-Help([string]$Topic) {
    $overview = @"

Release helper for iced-markdown-editor - binaries are built by GitHub Actions, release notes come from
$Changelog (Keep a Changelog + Semantic Versioning). This script does not write changelog
*content*; use the changelog skill/tool for that. It only promotes [Unreleased] to a dated
version section, keeps $CargoToml's version in sync, commits, tags, and pushes.

USAGE
  $Self <command> [version] [options]
  $Self <command> -h           detailed help for one command

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
  -Bump auto|patch|minor|major   Same as the version argument (default: auto)
  -Changelog <path>              Changelog file, relative to repo root (default: CHANGELOG.md)
  -CargoToml <path>              Cargo manifest, relative to repo root (default: Cargo.toml)
  -Remote <name>                 Git remote (default: origin)
  -Yes                           Skip confirmation prompts
  -h, -Help                      Show help

EXAMPLES (latest release v1.2.3)
  $Self rc                   test build  -> v1.3.0-rc.1
  $Self new                  release     -> v1.3.0
  $Self new patch            release     -> v1.2.4   (only last number)
  $Self new minor            release     -> v1.3.0   (second last number)
  $Self rc minor             test build  -> v1.3.0-rc.1
  $Self rollback v1.3.0-rc.1
  $Self retag -Yes

"@

    $details = @{
        'list' = @"

list
  Syncs tags with the remote and shows the latest 15 tags with date and subject.

"@
        'rc' = @"

rc [patch|minor|major|X.Y.Z] [-Yes]
  Creates the next release candidate tag vX.Y.Z-rc.N and pushes it.
  - Version is chosen like for 'new'; N counts up per version (rc.1, rc.2, ...).
  - Tag message = [Unreleased] section (or [X.Y.Z] if it exists). Empty is allowed.
  - $Changelog and $CargoToml are NOT modified, nothing is committed.
  - The GitHub Actions release workflow aborts the build unless the pushed tag's version
    matches $CargoToml's version and $Changelog's top section exactly (rc suffix included).
    Since this command changes neither file, the pipeline will usually fail its version check -
    this is only useful to sanity-check the tag/push mechanics, not to run a real test build.
    Use 'new' for a build that actually passes CI.

"@
        'new' = @"

new [patch|minor|major|X.Y.Z] [-Yes]
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

"@
        'rollback' = @"

rollback [tag] [-Yes]
  Deletes the tag on the remote and locally (default: highest tag, rc tags included).
  If the GitHub CLI ('gh') is installed and a GitHub Release exists for that tag, offers to
  delete it too. Without 'gh', delete the release manually if one was published. Built
  artifacts already downloaded by others stay untouched.
  $Changelog and $CargoToml are not touched: running 'new' again re-releases the same version.

"@
        'retag' = @"

retag [tag] [-Yes]
  Moves an existing tag (default: highest) to the current HEAD, keeps its message,
  and force-pushes it so the pipeline builds the same version again.
  Note: if a GitHub Release already exists for that tag, re-running the workflow may fail to
  publish over it depending on the release action's settings - check the run if that happens.

"@
        'show' = @"

show [tag]
  Prints the tag message and the commit it points to (default: highest tag).

"@
        'init' = @"

init [-Changelog <path>]
  Creates a CHANGELOG.md skeleton with an [Unreleased] section. Fails if it already exists.

"@
    }

    if ($Topic -and $details.ContainsKey($Topic)) { Write-Output $details[$Topic] }
    else { Write-Output $overview }
}

if ($Help -or $Command -eq 'help') {
    $topic = $null
    if ($Help -and $PSBoundParameters.ContainsKey('Command')) { $topic = $Command }
    elseif ($Command -eq 'help' -and $Version) { $topic = $Version }
    Show-Help $topic
    exit 0
}

# "new patch" / "rc minor" is shorthand for -Bump
if (($Command -in @('new', 'rc')) -and ($Version -in @('patch', 'minor', 'major'))) {
    $Bump = $Version.ToLower()
    $Version = $null
}

$ErrorActionPreference = 'Stop'
# Read git output as UTF-8 so umlauts survive
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)

# ============================================================== git helpers

function Invoke-Git {
    & git @args
    if ($LASTEXITCODE -ne 0) { throw "git $($args -join ' ') failed (exit code $LASTEXITCODE)" }
}

function Invoke-GitQuiet {
    # Runs git, ignores errors, returns $null on failure
    $out = & { $ErrorActionPreference = 'Continue'; & git @args 2>$null } @args
    if ($LASTEXITCODE -ne 0) { return $null }
    $out
}

function Confirm-Action([string]$Text) {
    if ($Yes) { return $true }
    $answer = Read-Host "$Text [y/N]"
    return $answer -match '^(y|yes|j|ja)$'
}

function Assert-Ready {
    $rel = $Changelog -replace '\\', '/'
    $dirty = @(Invoke-Git status --porcelain) | Where-Object { $_ -and $_.Substring(3) -ne $rel }
    if ($dirty) {
        Write-Warning "Uncommitted changes - they will NOT be part of the build:`n$($dirty -join "`n")"
    }
    $ahead = Invoke-GitQuiet rev-list --count '@{u}..HEAD'
    if ($ahead -and [int]$ahead -gt 0) {
        Write-Warning "$ahead commit(s) not pushed to your branch yet. The tag still includes them."
    }
}

function Assert-CanCommit {
    $branch = Invoke-GitQuiet symbolic-ref --quiet --short HEAD
    if (-not $branch) { throw 'Detached HEAD - check out a branch to create a release.' }
    $behind = Invoke-GitQuiet rev-list --count 'HEAD..@{u}'
    if ($behind -and [int]$behind -gt 0) { throw "Branch $branch is $behind commit(s) behind $Remote. Pull first." }
    "$branch".Trim()
}

function Get-RepoWebUrl {
    $url = Invoke-GitQuiet remote get-url $Remote
    if (-not $url) { return $null }
    $url = "$url".Trim()
    if ($url -match '^git@([^:]+):(.+?)(\.git)?$')                 { return "https://$($Matches[1])/$($Matches[2])" }
    if ($url -match '^ssh://git@([^/:]+)(:\d+)?/(.+?)(\.git)?$')   { return "https://$($Matches[1])/$($Matches[3])" }
    if ($url -match '^https?://(?:[^@/]+@)?(.+?)(\.git)?$')        { return "https://$($Matches[1])" }
    $null
}

function New-AnnotatedTag([string]$Name, [string]$Text, [string]$Target = 'HEAD', [switch]$Force) {
    # --cleanup=whitespace keeps markdown headings like "### Added" (default mode would strip lines starting with #)
    $file = [System.IO.Path]::GetTempFileName()
    try {
        [System.IO.File]::WriteAllText($file, $Text, $Utf8NoBom)
        $gitArgs = @('tag', '-a', '--cleanup=whitespace', '-F', $file)
        if ($Force) { $gitArgs += '-f' }
        $gitArgs += @($Name, $Target)
        Invoke-Git @gitArgs
    }
    finally {
        Remove-Item $file -ErrorAction SilentlyContinue
    }
}

# ============================================================== versions

function ConvertTo-Version([string]$Text) {
    if ($Text -notmatch '^v?(\d+)\.(\d+)\.(\d+)(?:-rc\.(\d+))?$') { return $null }
    $major = [int]$Matches[1]; $minor = [int]$Matches[2]; $patch = [int]$Matches[3]
    $isRc = [bool]$Matches[4]
    $rc = 0
    if ($isRc) { $rc = [int]$Matches[4] }
    $core = "$major.$minor.$patch"
    $tag = "v$core"
    if ($isRc) { $tag += "-rc.$rc" }
    [pscustomobject]@{
        Tag     = $tag
        Core    = $core
        Major   = $major
        Minor   = $minor
        Patch   = $patch
        IsRc    = $isRc
        Rc      = $rc
        # rc versions sort before the final release of the same version
        SortKey = '{0:D6}.{1:D6}.{2:D6}.{3}.{4:D6}' -f $major, $minor, $patch, [int](-not $isRc), $rc
    }
}

function Get-VersionTags {
    @(Invoke-Git tag --list 'v*') |
        ForEach-Object { ConvertTo-Version $_ } |
        Sort-Object SortKey
}

function Get-LatestTag([switch]$FinalOnly) {
    $all = @(Get-VersionTags | Where-Object { -not $FinalOnly -or -not $_.IsRc })
    if ($all.Count -eq 0) { return $null }
    $all[-1]
}

function Resolve-TagName([string]$Name) {
    if (-not $Name) {
        $latest = Get-LatestTag
        if (-not $latest) { throw 'No version tags found.' }
        return $latest.Tag
    }
    if ($Name -notmatch '^v') { $Name = "v$Name" }
    if (-not (Invoke-Git tag --list $Name)) { throw "Tag $Name does not exist on $Remote." }
    $Name
}

function Step-Version($Base, [string]$Level) {
    $major = 0; $minor = 0; $patch = 0
    if ($Base) { $major = $Base.Major; $minor = $Base.Minor; $patch = $Base.Patch }
    switch ($Level) {
        'major' { $major++; $minor = 0; $patch = 0 }
        'minor' { $minor++; $patch = 0 }
        'patch' { $patch++ }
    }
    "$major.$minor.$patch"
}

# ============================================================== Cargo.toml

function Read-CargoToml([string]$Path) {
    if (-not (Test-Path $Path)) { throw "$Path not found." }
    $raw = [System.IO.File]::ReadAllText($Path)
    $newLine = if ($raw.Contains("`r`n")) { "`r`n" } else { "`n" }
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.AddRange([string[]]($raw -split "`r?`n"))
    [pscustomobject]@{ Path = $Path; Lines = $lines; NewLine = $newLine }
}

function Save-CargoToml($Doc) {
    [System.IO.File]::WriteAllText($Doc.Path, ($Doc.Lines -join $Doc.NewLine), $Utf8NoBom)
}

function Find-CargoVersionLine($Doc) {
    # matches the same way the release workflow reads it: grep '^version' | head -n 1
    for ($i = 0; $i -lt $Doc.Lines.Count; $i++) {
        if ($Doc.Lines[$i] -match '^version\s*=\s*"([^"]+)"') { return $i }
    }
    $null
}

function Get-CargoVersion($Doc) {
    $i = Find-CargoVersionLine $Doc
    if ($null -eq $i) { throw "No top-level 'version' field found in $($Doc.Path)." }
    $Doc.Lines[$i] -match '^version\s*=\s*"([^"]+)"' | Out-Null
    $Matches[1]
}

function Set-CargoVersion($Doc, [string]$NewVersion) {
    $i = Find-CargoVersionLine $Doc
    if ($null -eq $i) { throw "No top-level 'version' field found in $($Doc.Path)." }
    $Doc.Lines[$i] = "version = `"$NewVersion`""
}

# ============================================================== changelog

function Read-Changelog([string]$Path) {
    if (-not (Test-Path $Path)) { throw "$Path not found. Run '.\scripts\release.ps1 init' to create one." }
    $raw = [System.IO.File]::ReadAllText($Path)
    $newLine = if ($raw.Contains("`r`n")) { "`r`n" } else { "`n" }
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.AddRange([string[]]($raw -split "`r?`n"))
    [pscustomobject]@{ Path = $Path; Lines = $lines; NewLine = $newLine }
}

function Save-Changelog($Doc) {
    [System.IO.File]::WriteAllText($Doc.Path, ($Doc.Lines -join $Doc.NewLine), $Utf8NoBom)
}

function Find-Section($Doc, [string]$Name) {
    $lines = $Doc.Lines
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^##\s+\[([^\]]+)\]' -and ($Matches[1] -replace '^v', '') -eq $Name) {
            $end = $i + 1
            # section ends at the next "## " heading or at the link references at the bottom
            while ($end -lt $lines.Count -and $lines[$end] -notmatch '^##\s' -and $lines[$end] -notmatch '^\[[^\]]+\]:\s') {
                $end++
            }
            return [pscustomobject]@{ Start = $i; End = $end }
        }
    }
    $null
}

function Get-SectionBody($Doc, $Section) {
    $count = $Section.End - $Section.Start - 1
    if ($count -le 0) { return @() }
    $body = @($Doc.Lines.GetRange($Section.Start + 1, $count))
    $first = 0
    while ($first -lt $body.Count -and -not $body[$first].Trim()) { $first++ }
    $last = $body.Count - 1
    while ($last -ge $first -and -not $body[$last].Trim()) { $last-- }
    if ($first -gt $last) { return @() }
    $body[$first..$last]
}

function Remove-EmptySubsections([string[]]$Body) {
    # drops "### Xyz" headings that have no content below them
    $result = [System.Collections.Generic.List[string]]::new()
    $pending = $null
    foreach ($line in $Body) {
        if ($line -match '^###\s') {
            $pending = [System.Collections.Generic.List[string]]::new()
            $pending.Add($line)
            continue
        }
        if ($null -ne $pending) {
            if ($line -match '^\s*$') { $pending.Add($line); continue }
            $result.AddRange($pending)
            $pending = $null
        }
        $result.Add($line)
    }
    while ($result.Count -gt 0 -and -not $result[$result.Count - 1].Trim()) { $result.RemoveAt($result.Count - 1) }
    $result.ToArray()
}

function Test-HasEntries([string[]]$Body) {
    [bool](@($Body) -match '^\s*[-*]\s+\S')
}

function Get-AutoBump([string[]]$Body) {
    # auto never changes the major number - major releases only with an explicit "major"
    $types = @{}
    $current = ''
    foreach ($line in $Body) {
        if ($line -match '^###\s+(.+?)\s*$') { $current = $Matches[1].ToLower(); continue }
        if ($line -match '^\s*[-*]\s+\S') { $types[$current] = 1 + [int]$types[$current] }
    }
    $level = 'patch'
    if ($types['added'] -or $types['changed'] -or $types['deprecated'] -or $types['removed']) { $level = 'minor' }
    if ($types['removed'] -or (($Body -join "`n") -cmatch 'BREAKING')) {
        $level = 'minor'
        Write-Warning "[Unreleased] contains breaking changes or removals. Auto bump stays at minor - use 'major' if you want a new major version."
    }
    $level
}

function Resolve-Target($Doc, [string]$CurrentCargoVersion) {
    $latestFinal = Get-LatestTag -FinalOnly
    $currentVersion = ConvertTo-Version $CurrentCargoVersion
    if (-not $currentVersion) { throw "Cargo.toml has an unexpected version '$CurrentCargoVersion' (expected X.Y.Z)." }

    $unreleased = Find-Section $Doc 'Unreleased'
    $unreleasedBody = @()
    if ($unreleased) { $unreleasedBody = @(Get-SectionBody $Doc $unreleased) }

    $core = $null
    $reason = ''

    if ($Version) {
        $core = $Version -replace '^v', '' -replace '-rc\.\d+$', ''
        if (-not (ConvertTo-Version $core)) { throw "Invalid version '$Version' (expected X.Y.Z)." }
        $reason = 'given with -Version'
    }
    else {
        $topSection = $null
        foreach ($line in $Doc.Lines) {
            if ($line -match '^##\s+\[v?(\d+\.\d+\.\d+)\]') { $topSection = ConvertTo-Version $Matches[1]; break }
        }
        if ($topSection -and (-not $latestFinal -or $topSection.SortKey -gt $latestFinal.SortKey)) {
            $core = $topSection.Core
            $reason = "untagged section [$core] in $Changelog"
            if ($Bump -ne 'auto') { Write-Warning "'$Bump' ignored: $Changelog already contains the untagged section [$core]. Pass an explicit X.Y.Z to override." }
        }
        else {
            $level = $Bump
            if ($Bump -eq 'auto') { $level = Get-AutoBump $unreleasedBody }
            $core = Step-Version $currentVersion $level
            $reason = "$level bump ($(if ($Bump -eq 'auto') { 'auto from [Unreleased]' } else { 'requested' })) from Cargo.toml $CurrentCargoVersion"
        }
    }

    $section = Find-Section $Doc $core
    $body = $unreleasedBody
    $source = '[Unreleased]'
    if ($section) {
        $body = @(Get-SectionBody $Doc $section)
        $source = "[$core]"
    }

    [pscustomobject]@{
        Core              = $core
        Reason            = $reason
        Body              = $body
        Source            = $source
        HasOwnSection     = [bool]$section
        UnreleasedBody    = $unreleasedBody
        LatestFinal       = $latestFinal
    }
}

function Update-ChangelogForRelease($Doc, [string]$Core, [string]$PrevTag) {
    $lines = $Doc.Lines
    $tag = "v$Core"

    # [Unreleased] -> [X.Y.Z] - date, plus a fresh empty [Unreleased] on top
    $unreleased = Find-Section $Doc 'Unreleased'
    $clean = @(Remove-EmptySubsections @(Get-SectionBody $Doc $unreleased))
    $lines.RemoveRange($unreleased.Start + 1, $unreleased.End - $unreleased.Start - 1)
    $lines.InsertRange($unreleased.Start + 1, [string[]](@('') + $clean + @('')))
    $lines[$unreleased.Start] = "## [$Core] - $(Get-Date -Format 'yyyy-MM-dd')"
    $lines.InsertRange($unreleased.Start, [string[]]@('## [Unreleased]', ''))

    # compare links at the bottom (Keep a Changelog reference-link convention)
    $web = Get-RepoWebUrl
    if (-not $web) { return }

    $unreleasedLink = "[unreleased]: $web/compare/$tag...HEAD"
    if ($PrevTag) {
        $versionLink = "[$Core]: $web/compare/$PrevTag...$tag"
    }
    else {
        $versionLink = "[$Core]: $web/releases/tag/$tag"
    }

    $linkIndex = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^\[unreleased\]:\s') { $linkIndex = $i; break }
    }
    if ($linkIndex -ge 0) {
        $lines[$linkIndex] = $unreleasedLink
        $lines.Insert($linkIndex + 1, $versionLink)
    }
    else {
        $insertAt = $lines.Count
        while ($insertAt -gt 0 -and -not $lines[$insertAt - 1].Trim()) { $insertAt-- }
        $lines.InsertRange($insertAt, [string[]]@('', $unreleasedLink, $versionLink))
    }
}

function Format-TagMessage([string]$Title, [string[]]$Body) {
    $text = '(no changelog entries)'
    if (Test-HasEntries $Body) { $text = (Remove-EmptySubsections $Body) -join "`n" }
    "$Title`n`n$text`n"
}

function Assert-VersionConsistency([string]$ChangelogPath, [string]$CargoTomlPath) {
    # mirrors the version-match check in .github/workflows/release.yml
    $cargoVersion = Get-CargoVersion (Read-CargoToml $CargoTomlPath)
    $changelogLine = Get-Content $ChangelogPath | Where-Object { $_ -match '^##\s+\[[0-9]' } | Select-Object -First 1
    if (-not $changelogLine -or $changelogLine -notmatch '\[(\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?)\]') {
        throw "Could not find a versioned '## [X.Y.Z]' section at the top of $ChangelogPath."
    }
    $changelogVersion = $Matches[1]
    if ($cargoVersion -ne $changelogVersion) {
        throw "Version mismatch: $CargoTomlPath has '$cargoVersion' but $ChangelogPath's latest section is [$changelogVersion]. The GitHub Actions build would abort on this - fix it before tagging."
    }
    $changelogVersion
}

# ============================================================== GitHub release (optional, via gh CLI)

function Remove-GitHubRelease([string]$Tag) {
    $gh = Get-Command gh -ErrorAction SilentlyContinue
    if (-not $gh) {
        $web = Get-RepoWebUrl
        $where = if ($web) { "$web/releases/tag/$Tag" } else { 'GitHub' }
        Write-Warning "GitHub CLI ('gh') not found - if a GitHub Release was published for $Tag, delete it manually at $where."
        return
    }
    & gh release view $Tag *> $null
    if ($LASTEXITCODE -ne 0) { return }
    if (-not (Confirm-Action "A GitHub Release exists for $Tag. Delete it too?")) { return }
    & gh release delete $Tag --yes
    if ($LASTEXITCODE -ne 0) { Write-Warning "Failed to delete the GitHub Release for $Tag - remove it manually if needed." }
}

# ============================================================== main

try {
    $null = Invoke-Git rev-parse --is-inside-work-tree
    $root = "$(Invoke-Git rev-parse --show-toplevel)".Trim()
    $changelogPath = if ([System.IO.Path]::IsPathRooted($Changelog)) { $Changelog } else { Join-Path $root $Changelog }
    $cargoTomlPath = if ([System.IO.Path]::IsPathRooted($CargoToml)) { $CargoToml } else { Join-Path $root $CargoToml }
    $cargoLockPath = Join-Path $root 'Cargo.lock'

    if ($Command -ne 'init') {
        # local tags = remote tags (removes stale ones, updates moved ones)
        Invoke-Git fetch $Remote --prune --prune-tags --force --quiet
    }

    switch ($Command) {

        'list' {
            $lines = @(Invoke-Git for-each-ref refs/tags '--sort=-v:refname' '--count=15' '--format=%(refname:short)|%(creatordate:short)|%(contents:subject)')
            if (-not $lines) { Write-Host "No tags on $Remote yet."; break }
            $lines | ForEach-Object {
                $p = $_ -split '\|', 3
                [pscustomobject]@{ Tag = $p[0]; Date = $p[1]; Subject = $p[2] }
            } | Format-Table -AutoSize
        }

        'rc' {
            Assert-Ready
            $doc = Read-Changelog $changelogPath
            $cargoDoc = Read-CargoToml $cargoTomlPath
            $currentCargoVersion = Get-CargoVersion $cargoDoc
            $t = Resolve-Target $doc $currentCargoVersion
            if (Invoke-Git tag --list "v$($t.Core)") { throw "v$($t.Core) is already released. Use -Version for another version." }

            $n = 1 + [int](@(Get-VersionTags) | Where-Object { $_.IsRc -and $_.Core -eq $t.Core } |
                           Measure-Object -Property Rc -Maximum).Maximum
            $tag = "v$($t.Core)-rc.$n"
            $message = Format-TagMessage "Release candidate $tag" $t.Body

            Write-Host ''
            Write-Host "Tag:     $tag" -ForegroundColor Cyan
            Write-Host "Version: $($t.Reason)"
            Write-Host "Commit:  $(Invoke-Git log -1 --oneline)"
            Write-Host "Notes:   $($t.Source) from $Changelog ($Changelog and $CargoToml are not modified)"
            Write-Host ''
            Write-Host $message

            if ($currentCargoVersion -ne $t.Core) {
                Write-Warning "Cargo.toml is at $currentCargoVersion, not $($t.Core). The release workflow's version check compares the tag ($tag) against Cargo.toml and $Changelog exactly, so this build will likely fail validation. Use 'new' for a build that actually passes CI."
            }

            if (-not (Confirm-Action "Create and push $tag?")) { Write-Host 'Aborted.'; break }
            New-AnnotatedTag -Name $tag -Text $message
            Invoke-Git push $Remote "refs/tags/$tag"
            Write-Host "Pushed $tag - GitHub Actions should start." -ForegroundColor Green
        }

        'new' {
            Assert-Ready
            $doc = Read-Changelog $changelogPath
            $cargoDoc = Read-CargoToml $cargoTomlPath
            $currentCargoVersion = Get-CargoVersion $cargoDoc
            $t = Resolve-Target $doc $currentCargoVersion
            $tag = "v$($t.Core)"

            if (Invoke-Git tag --list $tag) { throw "Tag $tag already exists. Use 'retag $tag' to rebuild or 'rollback $tag' first." }
            if (-not (Test-HasEntries $t.Body)) {
                throw "No entries in $($t.Source) of $Changelog. Describe your changes there first (or use 'rc' for a test build)."
            }

            $promoteChangelog = -not $t.HasOwnSection
            $bumpCargo = $currentCargoVersion -ne $t.Core
            $needsCommit = $promoteChangelog -or $bumpCargo

            $branch = $null
            if ($needsCommit) { $branch = Assert-CanCommit }
            elseif (Test-HasEntries $t.UnreleasedBody) {
                Write-Warning "[Unreleased] also has entries - they stay there for the next release."
            }

            $prevTag = $null
            if ($t.LatestFinal) { $prevTag = $t.LatestFinal.Tag }
            $message = Format-TagMessage "Release $tag" $t.Body

            Write-Host ''
            Write-Host "Tag:       $tag  (previous release: $(if ($prevTag) { $prevTag } else { '-' }))" -ForegroundColor Cyan
            Write-Host "Version:   $($t.Reason)"
            Write-Host "Commit:    $(Invoke-Git log -1 --oneline)"
            Write-Host "Notes:     $($t.Source) from $Changelog"
            if ($bumpCargo) { Write-Host "Cargo.toml: $currentCargoVersion -> $($t.Core)" }
            if ($promoteChangelog) {
                Write-Host "Changelog: [Unreleased] -> [$($t.Core)] - $(Get-Date -Format 'yyyy-MM-dd')"
            }
            if ($needsCommit) { Write-Host "Commit:    'Release $tag' pushed to $branch" }
            Write-Host ''
            Write-Host $message

            if (-not (Confirm-Action "Release $tag?")) { Write-Host 'Aborted.'; break }

            $changedPaths = [System.Collections.Generic.List[string]]::new()

            if ($promoteChangelog) {
                Update-ChangelogForRelease $doc $t.Core $prevTag
                Save-Changelog $doc
                $changedPaths.Add($changelogPath)
            }

            if ($bumpCargo) {
                Set-CargoVersion $cargoDoc $t.Core
                Save-CargoToml $cargoDoc
                $changedPaths.Add($cargoTomlPath)

                if (Get-Command cargo -ErrorAction SilentlyContinue) {
                    Write-Host 'Updating Cargo.lock (cargo check)...'
                    & cargo check --quiet
                    if ($LASTEXITCODE -ne 0) { throw 'cargo check failed while refreshing Cargo.lock.' }
                    if (Test-Path $cargoLockPath) { $changedPaths.Add($cargoLockPath) }
                }
                else {
                    Write-Warning "cargo not found on PATH - Cargo.lock was not refreshed. Run 'cargo check' before pushing if Cargo.lock tracks the package version."
                }
            }

            Assert-VersionConsistency $changelogPath $cargoTomlPath | Out-Null

            if ($needsCommit) {
                Invoke-Git add -- @($changedPaths | Select-Object -Unique)
                Invoke-Git commit --quiet -m "Release $tag"
                Invoke-Git push --quiet $Remote "HEAD:refs/heads/$branch"
            }

            New-AnnotatedTag -Name $tag -Text $message
            Invoke-Git push $Remote "refs/tags/$tag"
            Write-Host "Released $tag - GitHub Actions should start the build." -ForegroundColor Green
        }

        'rollback' {
            $tag = Resolve-TagName $Version
            Write-Host ''
            Invoke-Git --no-pager show --no-patch $tag
            Write-Host ''
            Write-Warning 'Only the tag is removed here. A GitHub Release attached to it stays online unless removed separately (see below). Built artifacts already downloaded by others are untouched.'

            if (-not (Confirm-Action "Delete $tag locally and on ${Remote}?")) { Write-Host 'Aborted.'; break }
            Invoke-Git push $Remote --delete "refs/tags/$tag"
            Invoke-Git tag -d $tag
            Remove-GitHubRelease $tag

            $prev = Get-LatestTag
            Write-Host "Deleted $tag. Highest tag is now: $(if ($prev) { $prev.Tag } else { 'none' })" -ForegroundColor Green
            if (-not ($tag -match '-rc\.')) {
                Write-Host "The [$($tag.TrimStart('v'))] section stays in $Changelog and the version stays in $CargoToml - running 'new' again re-releases it."
            }
        }

        'retag' {
            Assert-Ready
            $tag = Resolve-TagName $Version
            $message = ((Invoke-Git tag --list '--format=%(contents)' $tag) -join "`n").TrimEnd() + "`n"

            Write-Host ''
            Write-Host "Tag:  $tag" -ForegroundColor Cyan
            Write-Host "From: $(Invoke-Git log -1 --oneline "$tag^{commit}")"
            Write-Host "To:   $(Invoke-Git log -1 --oneline HEAD)"
            Write-Host ''

            if (-not (Confirm-Action "Move $tag to HEAD and force-push? (triggers a new GitHub Actions run)")) { Write-Host 'Aborted.'; break }
            New-AnnotatedTag -Name $tag -Text $message -Force
            Invoke-Git push --force $Remote "refs/tags/$tag"
            Write-Host "Re-pushed $tag." -ForegroundColor Green
        }

        'show' {
            $tag = Resolve-TagName $Version
            Invoke-Git --no-pager show --no-patch $tag
        }

        'init' {
            if (Test-Path $changelogPath) { throw "$changelogPath already exists." }
            $template = @(
                '# Changelog'
                ''
                'All notable changes to this project are documented in this file.'
                'Format: Keep a Changelog (https://keepachangelog.com), versioning: Semantic Versioning (https://semver.org).'
                ''
                '## [Unreleased]'
                ''
                '### Added'
                ''
                '### Changed'
                ''
                '### Fixed'
                ''
            ) -join "`n"
            [System.IO.File]::WriteAllText($changelogPath, $template, $Utf8NoBom)
            Write-Host "Created $changelogPath - commit it together with scripts/release.ps1." -ForegroundColor Green
        }
    }
}
catch {
    Write-Host "Error: $_" -ForegroundColor Red
    exit 1
}
