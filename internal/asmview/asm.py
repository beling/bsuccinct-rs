#!/usr/bin/env python3
"""
Local assembly snapshot management and diff tool for `asmview`.
Allows saving snapshots, comparing current/named versions, showing assembly,
and watching for live changes during iterative optimization.
"""

import argparse
import os
import re
import subprocess
import sys
import tempfile
import time
from datetime import datetime
from pathlib import Path


def get_workspace_root() -> Path:
    cmd = ["git", "rev-parse", "--show-toplevel"]
    res = subprocess.run(cmd, capture_output=True, text=True, check=True)
    return Path(res.stdout.strip())


def get_snapshots_dir(workspace_root: Path) -> Path:
    snapshots_dir = workspace_root / "internal" / "asmview" / ".snapshots"
    snapshots_dir.mkdir(parents=True, exist_ok=True)
    return snapshots_dir


def list_asmview_functions(cwd: Path) -> list[str]:
    cmd = ["cargo", "asm", "-p", "asmview", "--lib"]
    res = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)

    functions = []
    pattern = re.compile(r'^\d+\s+"([^"]+)"')
    for line in res.stderr.splitlines():
        match = pattern.search(line.strip())
        if match:
            functions.append(match.group(1))

    if not functions:
        lib_rs = cwd / "internal" / "asmview" / "src" / "lib.rs"
        if lib_rs.exists():
            fn_pattern = re.compile(r'pub\s+extern\s+"C"\s+fn\s+([a-zA-Z0-9_]+)')
            functions = fn_pattern.findall(lib_rs.read_text())

    return functions


def get_function_asm(cwd: Path, fn_name: str) -> str:
    cmd = ["cargo", "asm", "-p", "asmview", "--lib", fn_name]
    res = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if res.returncode != 0:
        print(f"Warning: Failed to get asm for function '{fn_name}': {res.stderr}", file=sys.stderr)
        return ""
    return res.stdout


def get_all_asm(cwd: Path, filter_pattern: str | None = None) -> str:
    all_functions = list_asmview_functions(cwd)
    if filter_pattern:
        all_functions = [f for f in all_functions if filter_pattern in f]

    out = []
    for fn in sorted(all_functions):
        out.append(f"=== FUNCTION: {fn} ===")
        asm = get_function_asm(cwd, fn)
        out.append(asm)
        out.append("\n")
    return "\n".join(out)


def cmd_save(args, workspace_root: Path):
    snapshots_dir = get_snapshots_dir(workspace_root)
    name = args.name or "baseline"
    target_file = snapshots_dir / f"{name}.asm"

    print(f"Building and generating assembly for snapshot '{name}'...")
    asm_content = get_all_asm(workspace_root, args.function)
    target_file.write_text(asm_content)
    print(f"Saved snapshot to: {target_file}")


def cmd_show(args, workspace_root: Path):
    asm_content = get_all_asm(workspace_root, args.function)
    print(asm_content)


def cmd_list(args, workspace_root: Path):
    snapshots_dir = get_snapshots_dir(workspace_root)
    snapshots = sorted(snapshots_dir.glob("*.asm"))

    if not snapshots:
        print("No snapshots found in .snapshots/. Save one using: ./asm.py save [name]")
        return

    print(f"Available snapshots ({len(snapshots)}):")
    for s in snapshots:
        mtime = datetime.fromtimestamp(s.stat().st_mtime).strftime("%Y-%m-%d %H:%M:%S")
        size = s.stat().st_size
        print(f"  - {s.stem:<20} ({size:>6} bytes, modified: {mtime})")


def cmd_rm(args, workspace_root: Path):
    snapshots_dir = get_snapshots_dir(workspace_root)
    target_file = snapshots_dir / f"{args.name}.asm"

    if not target_file.exists():
        print(f"Error: Snapshot '{args.name}' does not exist.", file=sys.stderr)
        sys.exit(1)

    target_file.unlink()
    print(f"Deleted snapshot '{args.name}'.")


def run_diff(left_file: Path, right_file: Path, left_label: str, right_label: str):
    print("\n" + "=" * 80)
    print(f"DIFF: {left_label} (left) vs {right_label} (right)")
    print("=" * 80 + "\n")

    cmd = ["git", "diff", "--no-index", "--color=always", str(left_file), str(right_file)]
    res = subprocess.run(cmd, capture_output=True, text=True)

    if res.returncode == 0:
        print("No assembly differences found!")
    else:
        print(res.stdout)


def cmd_diff(args, workspace_root: Path):
    snapshots_dir = get_snapshots_dir(workspace_root)

    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir)

        if args.target2:
            # Diff between two existing snapshots
            file1 = snapshots_dir / f"{args.target1}.asm"
            file2 = snapshots_dir / f"{args.target2}.asm"

            if not file1.exists():
                print(f"Error: Snapshot '{args.target1}' does not exist.", file=sys.stderr)
                sys.exit(1)
            if not file2.exists():
                print(f"Error: Snapshot '{args.target2}' does not exist.", file=sys.stderr)
                sys.exit(1)

            run_diff(file1, file2, f"snapshot '{args.target1}'", f"snapshot '{args.target2}'")

        else:
            # Diff current state vs a snapshot (default: baseline)
            baseline_name = args.target1 or "baseline"
            baseline_file = snapshots_dir / f"{baseline_name}.asm"

            if not baseline_file.exists():
                print(f"Snapshot '{baseline_name}' not found. Generating and saving it now as '{baseline_name}'...")
                asm = get_all_asm(workspace_root, args.function)
                baseline_file.write_text(asm)
                print(f"Saved snapshot '{baseline_name}'. Modify code and run diff again to see changes.")
                return

            current_file = tmp_path / "current.asm"
            print("Generating assembly for CURRENT working directory...")
            current_asm = get_all_asm(workspace_root, args.function)
            current_file.write_text(current_asm)

            run_diff(baseline_file, current_file, f"snapshot '{baseline_name}'", "CURRENT")


def get_watched_files(workspace_root: Path) -> dict[Path, float]:
    files = {}
    for p in workspace_root.rglob("*.rs"):
        if "target" in p.parts:
            continue
        try:
            files[p] = p.stat().st_mtime
        except OSError:
            pass
    return files


def cmd_watch(args, workspace_root: Path):
    snapshots_dir = get_snapshots_dir(workspace_root)
    baseline_name = args.name or "baseline"
    baseline_file = snapshots_dir / f"{baseline_name}.asm"

    if not baseline_file.exists():
        print(f"Baseline '{baseline_name}' not found. Generating and saving initial snapshot...")
        asm = get_all_asm(workspace_root, args.function)
        baseline_file.write_text(asm)
        print(f"Saved baseline '{baseline_name}'.")

    print(f"\n[Watch Mode] Watching for changes in Rust source files (*.rs)...")
    print(f"Comparing against baseline: '{baseline_name}'")
    print("Press Ctrl+C to exit.\n")

    last_files = get_watched_files(workspace_root)

    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir)
        current_file = tmp_path / "current.asm"

        try:
            while True:
                time.sleep(1.0)
                current_files = get_watched_files(workspace_root)
                changed = [p for p, mtime in current_files.items() if last_files.get(p) != mtime]

                if changed:
                    last_files = current_files
                    print(f"\n[{datetime.now().strftime('%H:%M:%S')}] Detected change in: {', '.join(p.name for p in changed[:3])}...")
                    current_asm = get_all_asm(workspace_root, args.function)
                    current_file.write_text(current_asm)
                    run_diff(baseline_file, current_file, f"snapshot '{baseline_name}'", "CURRENT")
        except KeyboardInterrupt:
            print("\nStopped watch mode.")


def main():
    parser = argparse.ArgumentParser(
        description="Local assembly snapshot and diff tool for quick optimization feedback."
    )
    subparsers = parser.add_subparsers(dest="command", help="Command to execute")

    # save
    p_save = subparsers.add_parser("save", help="Save current assembly state as a snapshot (default: baseline)")
    p_save.add_argument("name", nargs="?", default="baseline", help="Snapshot name (default: baseline)")
    p_save.add_argument("-f", "--function", help="Filter by function name")

    # diff
    p_diff = subparsers.add_parser("diff", help="Compare current assembly with a snapshot, or two snapshots")
    p_diff.add_argument("target1", nargs="?", default="baseline", help="Snapshot name to compare against, or left snapshot")
    p_diff.add_argument("target2", nargs="?", default=None, help="Optional right snapshot name (if comparing two snapshots)")
    p_diff.add_argument("-f", "--function", help="Filter by function name")

    # show
    p_show = subparsers.add_parser("show", help="Show generated assembly for current code")
    p_show.add_argument("-f", "--function", help="Filter by function name")

    # list
    subparsers.add_parser("list", help="List all saved snapshots")

    # rm
    p_rm = subparsers.add_parser("rm", help="Delete a snapshot")
    p_rm.add_argument("name", help="Name of snapshot to delete")

    # watch
    p_watch = subparsers.add_parser("watch", help="Watch for source file changes and run diff automatically")
    p_watch.add_argument("name", nargs="?", default="baseline", help="Baseline snapshot name (default: baseline)")
    p_watch.add_argument("-f", "--function", help="Filter by function name")

    args = parser.parse_args()
    workspace_root = get_workspace_root()

    if not args.command or args.command == "diff":
        if not args.command:
            args.target1 = "baseline"
            args.target2 = None
            args.function = None
        cmd_diff(args, workspace_root)
    elif args.command == "save":
        cmd_save(args, workspace_root)
    elif args.command == "show":
        cmd_show(args, workspace_root)
    elif args.command == "list":
        cmd_list(args, workspace_root)
    elif args.command == "rm":
        cmd_rm(args, workspace_root)
    elif args.command == "watch":
        cmd_watch(args, workspace_root)


if __name__ == "__main__":
    main()
