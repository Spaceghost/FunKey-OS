#!/usr/bin/env python3
from __future__ import annotations

import runpy
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[2]
runpy.run_path(str(root / ".github/scripts/apply_usb_ssh_hardening.py"), run_name="__main__")

# GITHUB_TOKEN may commit ordinary repository content but cannot update workflow
# files. The connector will add the permanent CI invocation separately.
subprocess.run(
    ["git", "checkout", "HEAD", "--", ".github/workflows/iroh.yml"],
    cwd=root,
    check=True,
)
