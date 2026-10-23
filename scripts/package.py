"""Assemble release assets and dependency notices without external Python packages."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]


def metadata():
    return json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))


def inventory(data):
    return sorted([
        {"name": p["name"], "version": p["version"], "license": p["license"],
         "source": f'https://crates.io/crates/{p["name"]}/{p["version"]}', "repository": p.get("repository")}
        for p in data["packages"] if p["name"] != "trucli"
    ], key=lambda p: (p["name"], p["version"]))


def main():
    data = metadata()
    inv = json.dumps(inventory(data), indent=2, ensure_ascii=False) + "\n"
    if "--inventory" in sys.argv:
        (ROOT / "DEPENDENCIES.json").write_text(inv, encoding="utf-8", newline="\n")
        return
    if "--check-inventory" in sys.argv:
        assert (ROOT / "DEPENDENCIES.json").read_text(encoding="utf-8") == inv, "Regenerate DEPENDENCIES.json"
        return
    exe = ROOT / "target/x86_64-pc-windows-msvc/release/trucli.exe"
    assert exe.is_file(), "Build the Windows target first"
    version = subprocess.check_output([str(exe), "--version"], text=True).strip()
    assert version == f"trucli {VERSION}", version
    out = ROOT / "dist"
    out.mkdir(exist_ok=True)
    stage = out / "trucli-windows-x86_64"
    stage.mkdir(exist_ok=False)
    shutil.copy2(exe, stage / "trucli.exe")
    for name in ["LICENSE", "THIRD_PARTY_NOTICES.md", "DEPENDENCIES.json", "README.md", "README_zh-CN.md", "CHANGELOG.md", "USAGE.txt"]:
        shutil.copy2(ROOT / name, stage / name)
    for p in data["packages"]:
        if p["name"] == "trucli":
            continue
        crate = Path(p["manifest_path"]).parent
        found = []
        for file in crate.rglob("*"):
            if file.is_file() and any(term in file.name.lower() for term in ("license", "copying", "notice")):
                found.append(file)
        assert found, f'No license texts found for {p["name"]}'
        for file in found:
            dest = stage / "dependency-licenses" / f'{p["name"]}-{p["version"]}' / file.relative_to(crate)
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(file, dest)
    archive = out / "trucli-windows-x86_64.zip"
    with zipfile.ZipFile(archive, "x", zipfile.ZIP_DEFLATED, strict_timestamps=False) as z:
        for file in sorted(stage.rglob("*")):
            if file.is_file():
                z.write(file, file.relative_to(stage))
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    (out / "SHA256SUMS.txt").write_text(f"{checksum}  {archive.name}\n", encoding="ascii", newline="\n")
    changelog = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    section = changelog.split(f"## [{VERSION}]", 1)[1].split("\n## [", 1)[0]
    notes = f"# Trucli v{VERSION}\n\n" + section.split("\n", 1)[1].strip() + "\n\nUnsigned experimental Windows x86-64 build. Verify SHA256SUMS.txt before use.\n"
    (out / "release-notes.md").write_text(notes, encoding="utf-8", newline="\n")
    print(f"{archive.name}: {checksum}")


if __name__ == "__main__":
    main()
