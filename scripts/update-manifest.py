"""Writes latest.json, the manifest the in-app updater reads.

Usage: python scripts/update-manifest.py <artifacts-dir> <tag> <repo> <notes-file> <out-file>

Looks for signed installers in <artifacts-dir> (each with a .sig next to it,
as cargo-packager writes them when CARGO_PACKAGER_SIGN_PRIVATE_KEY is set)
and points each platform at its release asset on GitHub. Platforms without
a signature are left out, so an unsigned build never offers itself as an
update.
"""
import datetime
import json
import pathlib
import sys

# Updater target -> (file suffix of the installer, update format).
PLATFORMS = {
    "windows-x86_64": ("-setup.exe", "nsis"),
    "macos-aarch64": (".app.tar.gz", "app"),
    "linux-x86_64": (".AppImage", "appimage"),
}


def main(artifacts, tag, repo, notes_file, out_file):
    version = tag.removeprefix("v")
    files = [path for path in pathlib.Path(artifacts).rglob("*") if path.is_file()]
    platforms = {}
    for target, (suffix, update_format) in PLATFORMS.items():
        for installer in files:
            signature = installer.with_name(installer.name + ".sig")
            if installer.name.endswith(suffix) and signature.exists():
                platforms[target] = {
                    "url": f"https://github.com/{repo}/releases/download/{tag}/{installer.name}",
                    "signature": signature.read_text(encoding="utf-8").strip(),
                    "format": update_format,
                }
                break
    notes = pathlib.Path(notes_file).read_text(encoding="utf-8").strip() if pathlib.Path(notes_file).exists() else ""
    manifest = {
        "version": version,
        "notes": notes,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    pathlib.Path(out_file).write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"latest.json: {', '.join(platforms) or 'no signed installers'}")


if __name__ == "__main__":
    main(*sys.argv[1:6])
