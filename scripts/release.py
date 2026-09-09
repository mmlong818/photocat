#!/usr/bin/env python3
"""Cut a release: build, sign, assemble latest.json, and publish to GitHub.

The updater reads `latest.json` from the repository's newest release, so the
installer, its minisign signature and that manifest have to land on the same
release together. Doing it by hand is easy to get subtly wrong — an unsigned
installer, or a manifest whose URL does not match the uploaded file name — and
either mistake only shows up later as a silent failure to update.

Nothing is published unless you pass --publish. Without it the script builds
everything and leaves the artifacts in dist/, so you can look before shipping.

    python scripts/release.py                 # build only
    python scripts/release.py --publish       # build, then create the release

The signing key lives outside the repository, by default at
~/.tauri/photocat.key. Override with --key or TAURI_SIGNING_PRIVATE_KEY_PATH.
Without it the bundle carries no signature and the updater will reject it.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import urllib.parse
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONF = ROOT / "src-tauri" / "tauri.conf.json"
OUT = ROOT / "dist"
# Only x64 Windows is built here. Adding an arch means another entry in
# PLATFORMS and another build on that machine; the manifest merges by key.
PLATFORM_KEY = "windows-x86_64"


def die(message):
    print("error: " + message, file=sys.stderr)
    raise SystemExit(1)


def run(args, cwd=None, env=None):
    print("$ " + " ".join(str(a) for a in args))
    result = subprocess.run(args, cwd=cwd or ROOT, env=env, shell=False)
    if result.returncode != 0:
        die("command failed: " + " ".join(str(a) for a in args))


def config():
    with open(CONF, encoding="utf-8") as handle:
        return json.load(handle)


def signing_env(key_path):
    """Tauri signs the bundle only when it can read the private key."""
    env = dict(os.environ)
    if key_path:
        if not key_path.is_file():
            die("signing key not found: %s" % key_path)
        env["TAURI_SIGNING_PRIVATE_KEY"] = key_path.read_text(encoding="utf-8").strip()
        env.setdefault("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "")
    return env


def build(env):
    """Build the frontend, the binary, then wrap it in the NSIS installer.

    The three steps are run separately on purpose. The config's
    beforeBuildCommand resolves `../src-vite` against the wrong directory when
    the CLI is invoked locally, so driving the frontend build here avoids it.
    """
    run(["pnpm", "--dir", "src-vite", "build"])
    run(["cargo", "build", "--release"], cwd=ROOT / "src-tauri", env=env)
    run(["cargo", "tauri", "bundle", "--bundles", "nsis"], cwd=ROOT / "src-tauri", env=env)


def collect(version, signed):
    """Find the installer the bundler just produced, with its signature."""
    nsis = ROOT / "src-tauri" / "target" / "release" / "bundle" / "nsis"
    installers = sorted(nsis.glob("*_%s_x64-setup.exe" % version))
    if not installers:
        die("no installer for version %s in %s" % (version, nsis))
    installer = installers[-1]

    signature = installer.with_suffix(installer.suffix + ".sig")
    if signed and not signature.is_file():
        die("installer is not signed: %s missing" % signature.name)
    return installer, (signature if signature.is_file() else None)


def portable_zip(version, product):
    """The portable build: the bare executable plus what it needs beside it."""
    resources = ROOT / "src-tauri" / "resources"
    binary = ROOT / "src-tauri" / "target" / "release" / "Lap.exe"
    if not binary.is_file():
        die("release binary not found: %s" % binary)

    name = "%s-便携版-%s" % (product, version)
    target = OUT / (name + ".zip")
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.write(binary, "%s/%s.exe" % (name, product))
        for folder in ("ffmpeg", "models"):
            source = resources / folder
            if not source.is_dir():
                die("missing portable resource folder: %s" % source)
            for path in sorted(source.rglob("*")):
                if path.is_file():
                    archive.write(path, "%s/%s" % (name, path.relative_to(resources)))
        # The marker that tells the running app not to install updates in place.
        archive.writestr("%s/portable.txt" % name, PORTABLE_README)
    return target


PORTABLE_README = (
    "这是便携版，直接双击 exe 运行，无需安装。\r\n"
    "本文件的存在告诉程序自己是便携版：检查到新版本时会引导你手动下载，\r\n"
    "而不是运行安装程序（那会在别处装出第二份，你这个目录不会被更新）。\r\n"
    "删除本文件会让程序把自己当作安装版。\r\n"
)


def manifest(version, installer, signature, notes, repo):
    """The document the updater fetches to decide whether to offer an update."""
    base = "%s/releases/download/v%s/" % (repo, version)
    return {
        "version": version,
        "notes": notes,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {
            PLATFORM_KEY: {
                # The file name carries non-ASCII characters, so the URL in the
                # manifest has to be percent-encoded or the download 404s.
                "url": base + urllib.parse.quote(installer.name),
                "signature": signature.read_text(encoding="utf-8").strip() if signature else "",
            }
        },
    }


def publish(version, files, notes, draft):
    tag = "v" + version
    args = ["gh", "release", "create", tag, "--title", tag, "--notes", notes]
    if draft:
        args.append("--draft")
    run(args + [str(path) for path in files])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--publish", action="store_true",
                        help="create the GitHub release and upload the artifacts")
    parser.add_argument("--draft", action="store_true",
                        help="publish as a draft release")
    parser.add_argument("--notes", default="", help="release notes")
    parser.add_argument("--key", default=os.environ.get("TAURI_SIGNING_PRIVATE_KEY_PATH")
                        or str(Path.home() / ".tauri" / "photocat.key"),
                        help="path to the minisign private key")
    parser.add_argument("--skip-build", action="store_true",
                        help="reuse the artifacts already in target/release")
    options = parser.parse_args()

    conf = config()
    version = conf["version"]
    product = conf["productName"]
    repo = conf["plugins"]["updater"]["endpoints"][0].split("/releases/")[0]
    notes = options.notes or ("%s %s" % (product, version))

    key_path = Path(options.key) if options.key else None
    env = signing_env(key_path)

    if not options.skip_build:
        build(env)

    OUT.mkdir(exist_ok=True)
    installer, signature = collect(version, signed=bool(key_path))
    archive = portable_zip(version, product)

    latest = OUT / "latest.json"
    with open(latest, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(manifest(version, installer, signature, notes, repo),
                  handle, ensure_ascii=False, indent=2)
        handle.write("\n")

    staged = [installer, latest, archive]
    if signature:
        staged.append(signature)
        shutil.copy2(signature, OUT / signature.name)
    shutil.copy2(installer, OUT / installer.name)

    print("\nversion %s, repository %s" % (version, repo))
    for path in staged:
        print("  %10.1f MB  %s" % (path.stat().st_size / 1048576, path.name))
    if not signature:
        print("\nWARNING: unsigned. The updater will refuse this build.")

    if options.publish:
        publish(version, staged, notes, options.draft)
        print("\npublished %s/releases/tag/v%s" % (repo, version))
    else:
        print("\nnothing published. Re-run with --publish when this looks right.")


if __name__ == "__main__":
    main()
