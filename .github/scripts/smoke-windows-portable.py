#!/usr/bin/env python3
"""Unpack the Windows portable zip, start the exe, and check it stays self-contained."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from pathlib import Path

BUNDLE_ID = "io.github.karimz1.oflh"
UNINSTALL_KEY = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OFLH Desktop"
CONTEXT_VERB = "oflh-desktop.inspect"
CONTEXT_CLASSES = ["*", "Directory", "Directory\\Background", "Drive"]


def registry_exists(hive: str, path: str) -> bool:
    result = subprocess.run(
        ["reg", "query", rf"{hive}\{path}"],
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def snapshot_state() -> dict:
    appdata = Path(os.environ["APPDATA"]) / BUNDLE_ID
    local = Path(os.environ["LOCALAPPDATA"]) / BUNDLE_ID
    return {
        "appdata_exists": appdata.exists(),
        "appdata_mtime": appdata.stat().st_mtime if appdata.exists() else None,
        "appdata_listing": listing(appdata),
        "local_exists": local.exists(),
        "local_mtime": local.stat().st_mtime if local.exists() else None,
        "local_listing": listing(local),
        "appdata_bundles": bundle_names(Path(os.environ["APPDATA"])),
        "local_bundles": bundle_names(Path(os.environ["LOCALAPPDATA"])),
        "uninstall": registry_exists("HKCU", UNINSTALL_KEY)
        or registry_exists("HKLM", UNINSTALL_KEY),
        "context": any(
            registry_exists("HKCU", rf"Software\Classes\{shell}\shell\{CONTEXT_VERB}")
            or registry_exists("HKLM", rf"Software\Classes\{shell}\shell\{CONTEXT_VERB}")
            for shell in CONTEXT_CLASSES
        ),
    }


def bundle_names(root: Path) -> set[str]:
    if not root.exists():
        return set()
    return {path.name for path in root.iterdir() if path.name.startswith(BUNDLE_ID)}


def listing(directory: Path) -> set[str]:
    if not directory.exists():
        return set()
    return {str(path.relative_to(directory)) for path in directory.rglob("*")}


def terminate(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    subprocess.run(
        ["taskkill", "/PID", str(process.pid), "/T", "/F"],
        capture_output=True,
        check=False,
    )
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: smoke-windows-portable.py <portable-zip>", file=sys.stderr)
        return 2
    archive = Path(sys.argv[1])
    if not archive.is_file() or archive.stat().st_size == 0:
        print(f"missing portable zip: {archive}", file=sys.stderr)
        return 1

    workspace = Path(tempfile.mkdtemp(prefix="oflh-portable-smoke-"))
    try:
        with zipfile.ZipFile(archive) as zipped:
            zipped.extractall(workspace)
        root = workspace / "oflh-desktop"
        exe = root / "oflh-desktop.exe"
        marker = root / "portable"
        data = root / "data"
        if not exe.is_file() or not marker.is_file():
            print(f"portable zip is missing the exe or marker under {root}", file=sys.stderr)
            return 1

        before = snapshot_state()
        process = subprocess.Popen(
            [str(exe)],
            cwd=str(root),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        sqlite = data / "oflh.sqlite3"
        webview = data / "webview"
        deadline = time.time() + 30
        try:
            while time.time() < deadline:
                if process.poll() is not None:
                    print(
                        f"portable app exited during startup with status {process.returncode}",
                        file=sys.stderr,
                    )
                    return 1
                if sqlite.is_file() and (webview.exists() or any(data.rglob("*"))):
                    break
                time.sleep(0.25)
            else:
                print("portable app did not write settings under data/ within 30s", file=sys.stderr)
                return 1
        finally:
            terminate(process)

        if not sqlite.is_file():
            print("recent-target database was not created under data/", file=sys.stderr)
            return 1
        after = snapshot_state()
        if after["appdata_exists"] and after["appdata_listing"] != before["appdata_listing"]:
            print(
                f"%APPDATA%\\{BUNDLE_ID} changed during the portable smoke test",
                file=sys.stderr,
            )
            return 1
        if after["local_exists"] and after["local_listing"] != before["local_listing"]:
            print(
                f"%LOCALAPPDATA%\\{BUNDLE_ID} changed during the portable smoke test",
                file=sys.stderr,
            )
            return 1
        if after["appdata_bundles"] != before["appdata_bundles"]:
            print(
                f"%APPDATA% gained or lost OFLH folders: {sorted(after['appdata_bundles'] - before['appdata_bundles'])}",
                file=sys.stderr,
            )
            return 1
        if after["local_bundles"] != before["local_bundles"]:
            print(
                f"%LOCALAPPDATA% gained or lost OFLH folders: {sorted(after['local_bundles'] - before['local_bundles'])}",
                file=sys.stderr,
            )
            return 1
        if after["uninstall"] and not before["uninstall"]:

            print("portable run created an uninstall registry key", file=sys.stderr)
            return 1
        if after["context"] and not before["context"]:
            print("portable run registered an Explorer context-menu verb", file=sys.stderr)
            return 1
        print(f"portable smoke ok: {sqlite} exists, app data locations unchanged")
        return 0
    finally:
        shutil.rmtree(workspace, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
