<#
.SYNOPSIS
  Tag-based release helper for iced-markdown-editor, driven by CHANGELOG.md (Keep a Changelog + Semantic Versioning)
  and Cargo.toml.

.DESCRIPTION
  Run from anywhere inside the repo. Pushing a tag vX.Y.Z (or vX.Y.Z-rc.N) starts
  .github/workflows/publish.yml, which publishes that version to crates.io. No GitHub release is
  created. The tag message comes from CHANGELOG.md - the "changelog" skill is responsible for
  writing the actual entries into [Unreleased]; this script only promotes that section to a dated
  version, keeps Cargo.toml's version in sync (the workflow aborts unless the tag matches
  Cargo.toml's version), commits, tags, and pushes.

  Commands
    list       Show the latest tags
    rc         Release candidate: like 'new', but renames [Unreleased] to [X.Y.Z-rc.N] - <date>,
               sets Cargo.toml to X.Y.Z-rc.N, tags vX.Y.Z-rc.N. Published to crates.io as a
               pre-release version.
    new        Release: renames [Unreleased] to [X.Y.Z] - <date> (after candidates: their entries
               plus [Unreleased]), syncs Cargo.toml's version, commits CHANGELOG.md/Cargo.toml/
               Cargo.lock, tags vX.Y.Z, pushes.
    rollback   Delete a tag locally and on the remote (default: highest tag); only useful if
               the version did not reach crates.io
    retag      Move a tag to HEAD and force-push it (retry a publish that failed)
    show       Show the message of a tag
    init       Create a CHANGELOG.md skeleton

  Version selection (new / rc)
    1. An explicit version: X.Y.Z
    2. A [X.Y.Z] section in CHANGELOG.md that is newer than the latest release tag (written by hand
       or left over from a rollback)
    3. While Cargo.toml is at a candidate X.Y.Z-rc.N: X.Y.Z (the next candidate, or the release)
    4. Otherwise Cargo.toml's current version is bumped:
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

Release helper for iced-markdown-editor - GitHub Actions publishes each pushed tag to crates.io
(no GitHub release), release notes come from $Changelog (Keep a Changelog + Semantic
Versioning). This script does not write changelog *content*; use the changelog skill/tool for
that. It only promotes [Unreleased] to a dated version section, keeps $CargoToml's version in
sync, commits, tags, and pushes.

USAGE
  $Self <command> [version] [options]
  $Self <command> -h           detailed help for one command

COMMANDS
  list                 Show the latest 15 tags (default)
  rc   [version]       Candidate: [Unreleased] -> [X.Y.Z-rc.N], sync Cargo.toml, commit, tag, push
  new  [version]       Release: [Unreleased] -> [X.Y.Z], sync Cargo.toml, commit, tag vX.Y.Z, push
  rollback [tag]       Delete a tag locally + on the remote (default: highest tag)
  retag    [tag]       Move a tag to HEAD and force-push it -> retry a failed publish
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
  $Self rc                   candidate   -> v1.3.0-rc.1, then rc.2, ...; new -> v1.3.0
  $Self new                  release     -> v1.3.0
  $Self new patch            release     -> v1.2.4   (only last number)
  $Self new minor            release     -> v1.3.0   (second last number)
  $Self rc minor             candidate   -> v1.3.0-rc.1
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
  Creates the next release candidate vX.Y.Z-rc.N. It is published to crates.io like a release,
  so it is recorded like one:
  - Version is chosen like for 'new' (while Cargo.toml is at X.Y.Z-rc.N: X.Y.Z again);
    N counts up per version (rc.1, rc.2, ...).
  - Renames [Unreleased] to [X.Y.Z-rc.N] - <today> (or uses that section if it is left over
    from a rollback), sets $CargoToml to X.Y.Z-rc.N, commits "Release vX.Y.Z-rc.N", pushes,
    tags with the section as message and pushes the tag.
  - Fails if the section has no entries: each candidate lists what changed since the last.
  - The publish workflow puts X.Y.Z-rc.N on crates.io. Cargo only picks a pre-release when a
    dependency asks for it (iced-markdown-editor = "X.Y.Z-rc.N"), so users of ^X.Y stay on
    the last release. Published versions cannot be deleted, only yanked.
  - 'new' later collects the entries of all candidates of X.Y.Z into [X.Y.Z].

"@
        'new' = @"

new [patch|minor|major|X.Y.Z] [-Yes]
  Creates a real release.
  1. Picks the version:
       - X.Y.Z if given
       - else an untagged [X.Y.Z] section in the changelog (hand-written or after a rollback)
       - else X.Y.Z while Cargo.toml is at a candidate X.Y.Z-rc.N
       - else bumps Cargo.toml's current version:
           patch -> 1.3.0 -> 1.3.1
           minor -> 1.3.0 -> 1.4.0
           major -> 1.3.0 -> 2.0.0  (only when given explicitly)
           none  -> auto from [Unreleased], never major:
                    ### Added / Changed / Deprecated / Removed or BREAKING  -> minor
                    anything else                                           -> patch
  2. If notes come from [Unreleased]: renames it to [X.Y.Z] - <today>, adds a new empty
     [Unreleased], and updates the compare links at the bottom of the changelog. After release
     candidates, [X.Y.Z] gets the entries of all [X.Y.Z-rc.N] sections plus [Unreleased],
     grouped by type; the candidates' sections stay below it as history.
  3. If Cargo.toml's version differs from X.Y.Z, updates it (and runs 'cargo check' to refresh
     Cargo.lock).
  4. Re-checks that Cargo.toml's version and the changelog's top section agree - the tag is
     named after the section, and the publish workflow aborts unless the tag matches
     Cargo.toml - before committing anything.
  5. Commits whatever changed above as "Release vX.Y.Z" and pushes the branch.
  6. Creates annotated tag vX.Y.Z with the section as message and pushes it. The publish
     workflow then tests the crate and publishes vX.Y.Z to crates.io.
  Fails if the section has no entries, the tag exists, or the branch is behind the remote.

"@
        'rollback' = @"

rollback [tag] [-Yes]
  Deletes the tag on the remote and locally (default: highest tag, rc tags included).
  Use it when the publish workflow failed before 'cargo publish', e.g. on a test failure.
  A version that reached crates.io cannot be deleted or published again: yank it with
  'cargo yank --version X.Y.Z' and release the next version instead.
  $Changelog and $CargoToml are not touched: running 'new' (or 'rc' for a candidate) again
  re-releases the same version.

"@
        'retag' = @"

retag [tag] [-Yes]
  Moves an existing tag (default: highest) to the current HEAD, keeps its message,
  and force-pushes it so the publish workflow runs again for the same version.
  Only useful if the version did not reach crates.io yet (the run failed before 'cargo
  publish'): crates.io rejects a version that is already published.

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
    # the first top-level version = "..." line, i.e. [package].version
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
    if (-not $currentVersion) { throw "Cargo.toml has an unexpected version '$CurrentCargoVersion' (expected X.Y.Z or X.Y.Z-rc.N)." }

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
        elseif ($currentVersion.IsRc -and $Bump -eq 'auto') {
            # the next candidate or the final release of the version being tested
            $core = $currentVersion.Core
            $reason = "release candidates of $core in progress (Cargo.toml $CurrentCargoVersion)"
        }
        else {
            $level = $Bump
            if ($Bump -eq 'auto') { $level = Get-AutoBump $unreleasedBody }
            # during release candidates, bump from the last release, not from the candidate
            $base = $currentVersion
            $from = "Cargo.toml $CurrentCargoVersion"
            if ($currentVersion.IsRc) {
                $base = $latestFinal
                $from = "the latest release $(if ($latestFinal) { $latestFinal.Tag } else { '(none)' })"
            }
            $core = Step-Version $base $level
            $reason = "$level bump ($(if ($Bump -eq 'auto') { 'auto from [Unreleased]' } else { 'requested' })) from $from"
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

function Get-CandidateSections($Doc, [string]$Core) {
    # the [X.Y.Z-rc.N] sections of a version, oldest first
    $found = foreach ($line in $Doc.Lines) {
        if ($line -match "^##\s+\[v?$([regex]::Escape($Core))-rc\.(\d+)\]") {
            $name = "$Core-rc.$($Matches[1])"
            [pscustomobject]@{ Name = $name; Rc = [int]$Matches[1]; Body = @(Get-SectionBody $Doc (Find-Section $Doc $name)) }
        }
    }
    @($found | Sort-Object Rc)
}

function Merge-Sections([object[]]$Bodies) {
    # one body with the entries of all, grouped by "### Type" in Keep a Changelog order
    $order = @('Added', 'Changed', 'Deprecated', 'Removed', 'Fixed', 'Security')
    $groups = [ordered]@{}
    foreach ($body in $Bodies) {
        $current = ''
        foreach ($line in @($body)) {
            if ($line -match '^###\s+(.+?)\s*$') { $current = $Matches[1]; continue }
            if (-not $line.Trim()) { continue }
            if (-not $groups.Contains($current)) { $groups[$current] = [System.Collections.Generic.List[string]]::new() }
            $groups[$current].Add($line)
        }
    }
    $names = @($groups.Keys | Where-Object { $_ -eq '' }) +
             @($order | Where-Object { $groups.Contains($_) }) +
             @($groups.Keys | Where-Object { $_ -and $_ -notin $order })
    $result = [System.Collections.Generic.List[string]]::new()
    foreach ($name in $names) {
        if ($result.Count -gt 0) { $result.Add('') }
        if ($name) { $result.Add("### $name") }
        $result.AddRange($groups[$name])
    }
    $result.ToArray()
}

function Update-ChangelogForRelease($Doc, [string]$Name, [string[]]$Body, [string]$PrevTag) {
    $lines = $Doc.Lines
    $tag = "v$Name"

    # [Unreleased] -> [Name] - date with the given entries, plus a fresh empty [Unreleased] on top
    $unreleased = Find-Section $Doc 'Unreleased'
    $clean = @(Remove-EmptySubsections $Body)
    $lines.RemoveRange($unreleased.Start + 1, $unreleased.End - $unreleased.Start - 1)
    $lines.InsertRange($unreleased.Start + 1, [string[]](@('') + $clean + @('')))
    $lines[$unreleased.Start] = "## [$Name] - $(Get-Date -Format 'yyyy-MM-dd')"
    $lines.InsertRange($unreleased.Start, [string[]]@('## [Unreleased]', ''))

    # compare links at the bottom (Keep a Changelog reference-link convention)
    $web = Get-RepoWebUrl
    if (-not $web) { return }

    $unreleasedLink = "[unreleased]: $web/compare/$tag...HEAD"
    if ($PrevTag) {
        $versionLink = "[$Name]: $web/compare/$PrevTag...$tag"
    }
    else {
        $versionLink = "[$Name]: $web/releases/tag/$tag"
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
    # the tag is named after the changelog section, and .github/workflows/publish.yml aborts
    # unless the tag matches Cargo.toml's version
    $cargoVersion = Get-CargoVersion (Read-CargoToml $CargoTomlPath)
    $changelogLine = Get-Content $ChangelogPath | Where-Object { $_ -match '^##\s+\[[0-9]' } | Select-Object -First 1
    if (-not $changelogLine -or $changelogLine -notmatch '\[(\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?)\]') {
        throw "Could not find a versioned '## [X.Y.Z]' section at the top of $ChangelogPath."
    }
    $changelogVersion = $Matches[1]
    if ($cargoVersion -ne $changelogVersion) {
        throw "Version mismatch: $CargoTomlPath has '$cargoVersion' but $ChangelogPath's latest section is [$changelogVersion]. The publish workflow would abort on this - fix it before tagging."
    }
    $changelogVersion
}

function Invoke-Release($Plan) {
    # Shared by 'rc' and 'new': shows the plan, then writes the changelog section and Cargo.toml's
    # version, commits, tags and pushes - so a candidate is recorded exactly like a release.
    $tag = $Plan.Tag
    $needsCommit = $Plan.Promote -or $Plan.BumpCargo
    $branch = $null
    if ($needsCommit) { $branch = Assert-CanCommit }

    Write-Host ''
    Write-Host "Tag:        $tag  (previous: $(if ($Plan.PrevTag) { $Plan.PrevTag } else { '-' }))" -ForegroundColor Cyan
    Write-Host "Version:    $($Plan.Reason)"
    Write-Host "Commit:     $(Invoke-Git log -1 --oneline)"
    Write-Host "Notes:      $($Plan.Source) from $Changelog"
    if ($Plan.BumpCargo) { Write-Host "Cargo.toml: $($Plan.CurrentCargoVersion) -> $($Plan.Name)" }
    if ($Plan.Promote) { Write-Host "Changelog:  [Unreleased] -> [$($Plan.Name)] - $(Get-Date -Format 'yyyy-MM-dd')" }
    if ($needsCommit) { Write-Host "Commit:     'Release $tag' pushed to $branch" }
    Write-Host ''
    Write-Host $Plan.Message

    if (-not (Confirm-Action "Release $tag?")) { Write-Host 'Aborted.'; return }

    $changedPaths = [System.Collections.Generic.List[string]]::new()
    if ($Plan.Promote) {
        Update-ChangelogForRelease $Plan.Doc $Plan.Name $Plan.Body $Plan.PrevTag
        Save-Changelog $Plan.Doc
        $changedPaths.Add($changelogPath)
    }
    if ($Plan.BumpCargo) {
        Set-CargoVersion $Plan.CargoDoc $Plan.Name
        Save-CargoToml $Plan.CargoDoc
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

    New-AnnotatedTag -Name $tag -Text $Plan.Message
    Invoke-Git push $Remote "refs/tags/$tag"
    Write-Host "Released $tag - GitHub Actions should start publishing to crates.io." -ForegroundColor Green
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
            $name = "$($t.Core)-rc.$n"
            $tag = "v$name"

            # a candidate's own section, left over from a rollback, is released as it is
            $own = Find-Section $doc $name
            if ($own) {
                $body = @(Get-SectionBody $doc $own)
                $source = "[$name]"
            }
            elseif ($t.HasOwnSection) {
                throw "$Changelog already has a [$($t.Core)] section. Release candidates take their notes from [Unreleased]; use 'new' to release $($t.Core)."
            }
            else {
                $body = $t.UnreleasedBody
                $source = '[Unreleased]'
            }
            if (-not (Test-HasEntries $body)) {
                throw "No entries in $source of $Changelog. Describe the changes in this candidate there first (or 'retag' to rebuild the last one)."
            }

            $prev = Get-LatestTag
            Invoke-Release ([pscustomobject]@{
                Tag                 = $tag
                Name                = $name
                Reason              = $t.Reason
                Body                = $body
                Source              = $source
                Promote             = -not $own
                BumpCargo           = $currentCargoVersion -ne $name
                CurrentCargoVersion = $currentCargoVersion
                PrevTag             = $(if ($prev) { $prev.Tag })
                Doc                 = $doc
                CargoDoc            = $cargoDoc
                Message             = Format-TagMessage "Release candidate $tag" $body
            })
        }

        'new' {
            Assert-Ready
            $doc = Read-Changelog $changelogPath
            $cargoDoc = Read-CargoToml $cargoTomlPath
            $currentCargoVersion = Get-CargoVersion $cargoDoc
            $t = Resolve-Target $doc $currentCargoVersion
            $tag = "v$($t.Core)"

            if (Invoke-Git tag --list $tag) { throw "Tag $tag already exists. Use 'retag $tag' to rebuild or 'rollback $tag' first." }

            # the release lists everything since the last one: its candidates' entries plus
            # [Unreleased]; the candidates' sections stay in the changelog as history
            $body = $t.Body
            $source = $t.Source
            $candidates = @()
            if (-not $t.HasOwnSection) { $candidates = @(Get-CandidateSections $doc $t.Core) }
            if ($candidates.Count -gt 0) {
                $body = Merge-Sections (@($candidates | ForEach-Object { , $_.Body }) + , $t.UnreleasedBody)
                $source = (@($candidates | ForEach-Object { "[$($_.Name)]" }) + '[Unreleased]') -join ' + '
            }
            if (-not (Test-HasEntries $body)) {
                throw "No entries in $source of $Changelog. Describe your changes there first (or use 'rc' for a test build)."
            }
            if ($t.HasOwnSection -and (Test-HasEntries $t.UnreleasedBody)) {
                Write-Warning "[Unreleased] also has entries - they stay there for the next release."
            }

            Invoke-Release ([pscustomobject]@{
                Tag                 = $tag
                Name                = $t.Core
                Reason              = $t.Reason
                Body                = $body
                Source              = $source
                Promote             = -not $t.HasOwnSection
                BumpCargo           = $currentCargoVersion -ne $t.Core
                CurrentCargoVersion = $currentCargoVersion
                PrevTag             = $(if ($t.LatestFinal) { $t.LatestFinal.Tag })
                Doc                 = $doc
                CargoDoc            = $cargoDoc
                Message             = Format-TagMessage "Release $tag" $body
            })
        }

        'rollback' {
            $tag = Resolve-TagName $Version
            Write-Host ''
            Invoke-Git --no-pager show --no-patch $tag
            Write-Host ''
            Write-Warning "Only the tag is removed here. If $($tag.TrimStart('v')) already reached crates.io, it stays there and cannot be published again - yank it with 'cargo yank --version $($tag.TrimStart('v'))' and release the next version instead."

            if (-not (Confirm-Action "Delete $tag locally and on ${Remote}?")) { Write-Host 'Aborted.'; break }
            Invoke-Git push $Remote --delete "refs/tags/$tag"
            Invoke-Git tag -d $tag

            $prev = Get-LatestTag
            Write-Host "Deleted $tag. Highest tag is now: $(if ($prev) { $prev.Tag } else { 'none' })" -ForegroundColor Green
            $again = if ($tag -match '-rc\.') { 'rc' } else { 'new' }
            Write-Host "The [$($tag.TrimStart('v'))] section stays in $Changelog and the version stays in $CargoToml - running '$again' again re-releases it."
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

            if (-not (Confirm-Action "Move $tag to HEAD and force-push? (publishes again - fails if the version is already on crates.io)")) { Write-Host 'Aborted.'; break }
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
