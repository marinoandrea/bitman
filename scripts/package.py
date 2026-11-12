#!/usr/bin/env python3
"""Package an already-built native binary; Cargo owns every package version."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def notices(destination):
    copy(ROOT / "README.md", destination / "README.md")
    for source in sorted((ROOT / "assets/fonts").glob("*/OFL.txt")):
        copy(source, destination / "licenses" / source.parent.name / source.name)


def macos_bundle(binary, destination, version):
    bundle = destination / "Bitman.app"
    copy(binary, bundle / "Contents/MacOS/bitman")
    copy(ROOT / "assets/icons/macos/Bitman.icns",
         bundle / "Contents/Resources/Bitman.icns")
    notices(bundle / "Contents/Resources")
    with (ROOT / "packaging/macos/Info.plist").open("rb") as source:
        info = plistlib.load(source)
    info["CFBundleVersion"] = version
    info["CFBundleShortVersionString"] = version
    with (bundle / "Contents/Info.plist").open("wb") as output:
        plistlib.dump(info, output)
    run("plutil", "-lint", str(bundle / "Contents/Info.plist"))
    run("codesign", "--force", "--sign", "-", str(bundle))
    run("codesign", "--verify", "--deep", "--strict", str(bundle))
    return bundle


def linux_packages(binary, stage, output, name, version):
    package = stage / name
    copy(binary, package / "usr/bin/bitman")
    copy(ROOT / "packaging/linux/bitman.desktop",
         package / "usr/share/applications/bitman.desktop")
    copy(ROOT / "assets/icons/app-icon.png", package / "usr/share/pixmaps/bitman.png")
    notices(package / "usr/share/doc/bitman")
    archive = output / f"{name}.tar.gz"
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(package, arcname=name)

    # Derive linked-library requirements from the actual ELF on the build distro.
    debian = stage / "debian"
    debian.mkdir()
    (debian / "control").write_text("Source: bitman\n\nPackage: bitman\nArchitecture: amd64\n")
    result = run("dpkg-shlibdeps", "-O", f"-e{binary}", cwd=stage, capture_output=True)
    dependencies = next(line.removeprefix("shlibs:Depends=")
                        for line in result.stdout.splitlines()
                        if line.startswith("shlibs:Depends="))
    # GPUI also loads the graphics/window libraries dynamically.
    dependencies += ", libvulkan1, libxkbcommon-x11-0, libwayland-client0, libwayland-cursor0"
    control = package / "DEBIAN/control"
    control.parent.mkdir()
    control.write_text(
        f"Package: bitman\nVersion: {version}\nArchitecture: amd64\n"
        "Maintainer: Andrea Marino <a.marino@uva.nl>\n"
        "Section: graphics\nPriority: optional\n"
        f"Installed-Size: {sum(p.stat().st_size for p in package.rglob('*') if p.is_file()) // 1024}\n"
        f"Depends: {dependencies}\nRecommends: xdg-desktop-portal\n"
        "Homepage: https://github.com/marinoandrea/bitman\n"
        "Description: Native image processing and print workspace\n"
    )
    deb = output / f"{name}.deb"
    run("dpkg-deb", "--root-owner-group", "--build", str(package), str(deb))
    run("dpkg-deb", "--info", str(deb))
    return [archive, deb]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", help="Rust target triple used by cargo build --target")
    parser.add_argument("--profile", choices=["debug", "release"], default="release")
    parser.add_argument("--bundle-only", action="store_true", help="Create only the local macOS app")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    os.chdir(ROOT)
    host = next(line.removeprefix("host: ")
                for line in run("rustc", "-vV", capture_output=True).stdout.splitlines()
                if line.startswith("host: "))
    target = args.target or host
    supported = {"aarch64-apple-darwin": "Darwin", "x86_64-apple-darwin": "Darwin",
                 "x86_64-unknown-linux-gnu": "Linux", "x86_64-pc-windows-msvc": "Windows"}
    if supported.get(target) != platform.system():
        parser.error(f"{target} must be packaged on its native operating system")
    if args.bundle_only and platform.system() != "Darwin":
        parser.error("--bundle-only requires macOS")
    if not args.bundle_only and args.profile != "release":
        parser.error("Distributable packages require --profile release")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
    build_dir = target_dir / target / args.profile if args.target else target_dir / args.profile
    binary = build_dir / ("bitman.exe" if platform.system() == "Windows" else "bitman")
    if not binary.is_file():
        parser.error(f"Build the executable first: {binary}")
    actual = run(str(binary), "--version", capture_output=True).stdout.strip()
    if actual != f"bitman {version}":
        parser.error(f"Stale executable: expected bitman {version}, got {actual!r}")
    if args.bundle_only:
        print(macos_bundle(binary, build_dir, version))
        return

    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    name = f"bitman-{version}-{target}"
    with tempfile.TemporaryDirectory(prefix="bitman-package-") as temp:
        stage = Path(temp)
        if platform.system() == "Darwin":
            bundle = macos_bundle(binary, stage, version)
            archive = output / f"{name}.zip"
            run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(bundle), str(archive))
            (stage / "Applications").symlink_to("/Applications", target_is_directory=True)
            dmg = output / f"{name}.dmg"
            run("hdiutil", "create", "-volname", "Bitman", "-srcfolder", str(stage),
                "-ov", "-format", "UDZO", str(dmg))
            run("hdiutil", "verify", str(dmg))
            artifacts = [archive, dmg]
        elif platform.system() == "Linux":
            artifacts = linux_packages(binary, stage, output, name, version)
        else:
            package = stage / name
            copy(binary, package / "bitman.exe")
            notices(package)
            artifacts = [Path(shutil.make_archive(str(output / name), "zip", stage, name))]

    for artifact in artifacts:
        with artifact.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        artifact.with_name(artifact.name + ".sha256").write_text(f"{digest}  {artifact.name}\n")
        print(artifact)
    (output / f"{name}.json").write_text(json.dumps({
        "version": version, "target": target,
        "commit": run("git", "rev-parse", "HEAD", capture_output=True).stdout.strip(),
        "artifacts": [artifact.name for artifact in artifacts],
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
