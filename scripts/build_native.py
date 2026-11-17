#!/usr/bin/env python3
"""Build and stage native runtime libraries using Cargo's reported output paths."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys


LIBRARIES = {
    "pano_native_components": {"pano_native_components.dll", "libpano_native_components.so"},
    "native_webview": {"native_webview.dll", "libnative_webview.so"},
}


def stage_artifacts(messages: list[dict], destination: Path) -> list[Path]:
    outputs = {}
    libraries = {}
    for message in messages:
        if message.get("reason") == "build-script-executed":
            outputs[message["package_id"]] = Path(message["out_dir"])
        elif message.get("reason") == "compiler-artifact":
            target = message["target"]
            name = target["name"]
            if name in LIBRARIES and "cdylib" in target["kind"]:
                paths = [Path(p) for p in message["filenames"] if Path(p).name in LIBRARIES[name]]
                if len(paths) != 1:
                    raise ValueError(f"Expected one supported runtime library for {name}.")
                libraries[name] = (paths[0], message["package_id"])

    if libraries.keys() != LIBRARIES.keys():
        raise ValueError("Cargo did not report both native runtime libraries.")

    files = [libraries[name][0] for name in LIBRARIES]
    webview, package_id = libraries["native_webview"]
    if webview.suffix == ".dll":
        if package_id not in outputs:
            raise ValueError("Cargo did not report the native-webview build-script output directory.")
        files.append(outputs[package_id] / "WebView2Loader.dll")

    for source in files:
        if not source.is_file() or source.stat().st_size == 0:
            raise ValueError(f"Required runtime library is missing or empty: {source}")

    destination.mkdir(parents=True, exist_ok=True)
    staged = []
    for source in files:
        target = destination / source.name
        if source.resolve() != target.resolve():
            shutil.copy2(source, target)
        staged.append(target)
    return staged


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    args, cargo_args = parser.parse_known_args()
    root = Path(__file__).resolve().parents[1]
    result = subprocess.run(
        ["cargo", "build", "--workspace", "--message-format=json", *cargo_args],
        cwd=root, stdout=subprocess.PIPE, text=True, encoding="utf-8",
    )
    try:
        messages = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
        for message in messages:
            if message.get("reason") == "compiler-message":
                print(message["message"].get("rendered") or message["message"]["message"], file=sys.stderr, end="")
        if result.returncode:
            return result.returncode
        for path in stage_artifacts(messages, root / args.output_dir):
            print(f"Staged {path}")
    except (OSError, ValueError, KeyError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
