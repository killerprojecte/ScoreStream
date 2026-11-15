#!/usr/bin/env python3
"""Build then run opt-in native tests without Cargo's injected loader search path.

This helper never removes or changes caller loader settings. If LD_* is set,
fix the launching environment explicitly rather than bypassing the native guard.
Python is test orchestration only; compilation, scheduling and PCM are Rust.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

if any(key.startswith("LD_") and value for key, value in os.environ.items()):
    sys.exit("Refusing native tests with nonempty LD_* overrides.")
manifest = Path(__file__).resolve().parents[1] / "Cargo.toml"
build = subprocess.run(
    ["cargo", "test", "--manifest-path", str(manifest), "--offline", "--locked",
     "--test", "native", "--no-run", "--message-format=json"],
    text=True, stdout=subprocess.PIPE, check=True,
)
executables = []
for line in build.stdout.splitlines():
    item = json.loads(line)
    if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "native":
        if item.get("executable"):
            executables.append(item["executable"])
if len(executables) != 1:
    sys.exit("Expected exactly one native test executable from Cargo.")
subprocess.run([executables[0], "--ignored", "--test-threads=1", *sys.argv[1:]], check=True)
