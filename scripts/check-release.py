#!/usr/bin/env python3
"""Fail CI when release-managed versions or package templates drift."""

import json
from pathlib import Path
import plistlib
import re
import tomllib

ROOT = Path(__file__).resolve().parent.parent
package = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]
version = package["version"]
assert re.fullmatch(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)", version), version
lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
assert next(p["version"] for p in lock["package"] if p["name"] == "bitman") == version
manifest = json.loads((ROOT / ".release-please-manifest.json").read_text())
# Empty until release-please prepares the first (0.1.0) release PR.
assert manifest.get(".", "0.1.0") == version
with (ROOT / "packaging/macos/Info.plist").open("rb") as source:
    plist = plistlib.load(source)
assert plist["CFBundleVersion"] == plist["CFBundleShortVersionString"] == "@VERSION@"
print(f"Release metadata is consistent: {version}")
