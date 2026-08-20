#!/usr/bin/env python3
"""
Local assembly snapshot management and diff tool for `asmview`.
Allows saving snapshots, comparing current/named versions, showing assembly,
watching for live changes, and fetching/caching snapshots from Git revisions (git_<ref>).
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


def get_all_asm(cwd: Path) -> str:
    """Always generates assembly for all functions without filtering."""
    all_functions = list_asmview_functions(cwd)
    out = []
    for fn in sorted(all_functions):
        out.append(f"=== FUNCTION: {fn} ===")
        asm = get_function_asm(cwd, fn)
        out.append(asm)
        out.append("\n")
    return "\n".join(out)


def filter_asm(asm_text: str, filter_pattern: str | None = None) -> str:
    """Filters assembly blocks by matching function names against filter_pattern."""
    if not filter_pattern:
        return asm_text

    blocks = asm_text.split("=== FUNCTION: ")
    filtered_blocks = []
    for block in blocks:
        if not block.strip():
            continue
        first_line, _, rest = block.partition(" ===")
        fn_name = first_line.strip()
        if filter_pattern in fn_name:
            filtered_blocks.append(f"=== FUNCTION: {fn_name} ==={rest}")

    return "\n".join(filtered_blocks)


def resolve_git_ref(workspace_root: Path, ref: str) -> str:
    """Resolve a git reference (branch, tag, relative ref like HEAD~1) to a short commit hash."""
    cmd = ["git", "rev-parse", "--short=10", "--verify", ref]
    res = subprocess.run(cmd, cwd=workspace_root, capture_output=True, text=True)
    if res.returncode != 0:
        print(f"Error: Git revision '{ref}' not found:\n{res.stderr.strip()}", file=sys.stderr)
        sys.exit(1)
    return res.stdout.strip()


def ensure_git_snapshot(workspace_root: Path, name: str, print_if_exists: bool = False) -> tuple[str, Path]:
    """
    If `name` starts with 'git_', resolves the git reference to a commit hash 'git_<commit_hash>',
    creates the full snapshot file via a temporary git worktree if it does not already exist,
    and returns (canonical_name, Path to the snapshot file).
    Otherwise, returns (name, Path to the regular snapshot file in .snapshots/).
    """
    snapshots_dir = get_snapshots_dir(workspace_root)
    if not name.startswith("git_"):
        return name, snapshots_dir / f"{name}.asm"

    raw_ref = name[len("git_"):]
    commit_hash = resolve_git_ref(workspace_root, raw_ref)
    canonical_name = f"git_{commit_hash}"
    target_file = snapshots_dir / f"{canonical_name}.asm"

    if target_file.exists():
        if print_if_exists:
            print(f"Snapshot '{canonical_name}' already exists.")
        return canonical_name, target_file

    print(f"Generating snapshot '{canonical_name}' for Git revision '{raw_ref}' ({commit_hash})...")
    with tempfile.TemporaryDirectory() as tmp_dir:
        worktree_dir = Path(tmp_dir) / "wt"
        cmd_worktree = ["git", "worktree", "add", "--detach", str(worktree_dir), commit_hash]
        res_wt = subprocess.run(cmd_worktree, cwd=workspace_root, capture_output=True, text=True)
        if res_wt.returncode != 0:
            print(f"Error creating git worktree for revision '{commit_hash}':\n{res_wt.stderr}", file=sys.stderr)
            sys.exit(1)

        try:
            asm_content = get_all_asm(worktree_dir)
            target_file.write_text(asm_content)
            print(f"Saved snapshot to: {target_file}")
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", str(worktree_dir)], cwd=workspace_root, capture_output=True)

    return canonical_name, target_file


def cmd_save(args, workspace_root: Path):
    snapshots_dir = get_snapshots_dir(workspace_root)
    name = args.name or "baseline"

    if name.startswith("git_"):
        ensure_git_snapshot(workspace_root, name, print_if_exists=True)
    else:
        target_file = snapshots_dir / f"{name}.asm"
        print(f"Building and generating assembly for snapshot '{name}'...")
        asm_content = get_all_asm(workspace_root)
        target_file.write_text(asm_content)
        print(f"Saved snapshot to: {target_file}")


def cmd_show(args, workspace_root: Path):
    if args.name:
        _, snapshot_file = ensure_git_snapshot(workspace_root, args.name)
        if not snapshot_file.exists():
            print(f"Error: Snapshot '{args.name}' does not exist.", file=sys.stderr)
            sys.exit(1)
        asm_content = snapshot_file.read_text()
    else:
        asm_content = get_all_asm(workspace_root)

    print(filter_asm(asm_content, args.function))


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
        print(f"  - {s.stem:<25} ({size:>6} bytes, modified: {mtime})")


def cmd_rm(args, workspace_root: Path):
    snapshots_dir = get_snapshots_dir(workspace_root)
    name = args.name

    if name.startswith("git_"):
        raw_ref = name[len("git_"):]
        try:
            commit_hash = resolve_git_ref(workspace_root, raw_ref)
            canonical_name = f"git_{commit_hash}"
        except SystemExit:
            canonical_name = name
        target_file = snapshots_dir / f"{canonical_name}.asm"
        if not target_file.exists():
            print(f"Snapshot '{canonical_name}' does not exist, nothing to remove.")
            return
    else:
        target_file = snapshots_dir / f"{name}.asm"
        if not target_file.exists():
            print(f"Error: Snapshot '{name}' does not exist.", file=sys.stderr)
            sys.exit(1)

    target_file.unlink()
    print(f"Deleted snapshot '{target_file.stem}'.")


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
            # Diff between two snapshots (can be regular or git_<ref>)
            name1, file1 = ensure_git_snapshot(workspace_root, args.target1)
            name2, file2 = ensure_git_snapshot(workspace_root, args.target2)

            if not file1.exists():
                print(f"Error: Snapshot '{args.target1}' does not exist.", file=sys.stderr)
                sys.exit(1)
            if not file2.exists():
                print(f"Error: Snapshot '{args.target2}' does not exist.", file=sys.stderr)
                sys.exit(1)

            asm1 = filter_asm(file1.read_text(), args.function)
            asm2 = filter_asm(file2.read_text(), args.function)

            tmp_file1 = tmp_path / f"{name1}.asm"
            tmp_file2 = tmp_path / f"{name2}.asm"
            tmp_file1.write_text(asm1)
            tmp_file2.write_text(asm2)

            run_diff(tmp_file1, tmp_file2, f"snapshot '{name1}'", f"snapshot '{name2}'")

        else:
            # Diff current state vs a snapshot (default: baseline)
            baseline_name = args.target1 or "baseline"
            canonical_name, baseline_file = ensure_git_snapshot(workspace_root, baseline_name)

            if not baseline_file.exists():
                print(f"Snapshot '{baseline_name}' not found. Generating and saving it now as '{baseline_name}'...")
                asm = get_all_asm(workspace_root)
                baseline_file.write_text(asm)
                print(f"Saved snapshot '{baseline_name}'. Modify code and run diff again to see changes.")
                return

            baseline_asm = filter_asm(baseline_file.read_text(), args.function)
            tmp_baseline_file = tmp_path / f"{canonical_name}.asm"
            tmp_baseline_file.write_text(baseline_asm)

            current_file = tmp_path / "current.asm"
            print("Generating assembly for CURRENT working directory...")
            current_asm = filter_asm(get_all_asm(workspace_root), args.function)
            current_file.write_text(current_asm)

            run_diff(tmp_baseline_file, current_file, f"snapshot '{canonical_name}'", "CURRENT")


def get_watched_files(workspace_root: Path) -> dict[Path, float]:
    files = {}
    for p in workspace_root.rglob("*.rs"):
        if "target" in p.parts or ".snapshots" in p.parts:
            continue
        try:
            files[p] = p.stat().st_mtime
        except OSError:
            pass
    return files


def cmd_watch(args, workspace_root: Path):
    baseline_name = args.name or "baseline"
    canonical_name, baseline_file = ensure_git_snapshot(workspace_root, baseline_name)

    if not baseline_file.exists():
        print(f"Baseline '{baseline_name}' not found. Generating and saving initial snapshot...")
        asm = get_all_asm(workspace_root)
        baseline_file.write_text(asm)
        print(f"Saved baseline '{baseline_name}'.")

    print(f"\n[Watch Mode] Watching for changes in Rust source files (*.rs)...")
    print(f"Comparing against baseline: '{canonical_name}'")
    if args.function:
        print(f"Filtering function: '{args.function}'")
    print("Press Ctrl+C to exit.\n")

    last_files = get_watched_files(workspace_root)

    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir)
        tmp_baseline_file = tmp_path / f"{canonical_name}.asm"
        tmp_current_file = tmp_path / "current.asm"

        try:
            while True:
                time.sleep(1.0)
                current_files = get_watched_files(workspace_root)
                changed = [p for p, mtime in current_files.items() if last_files.get(p) != mtime]

                if changed:
                    last_files = current_files
                    print(f"\n[{datetime.now().strftime('%H:%M:%S')}] Detected change in: {', '.join(p.name for p in changed[:3])}...")
                    
                    # Refresh baseline content (with filter)
                    baseline_asm = filter_asm(baseline_file.read_text(), args.function)
                    tmp_baseline_file.write_text(baseline_asm)

                    # Get and filter current assembly
                    current_asm = filter_asm(get_all_asm(workspace_root), args.function)
                    tmp_current_file.write_text(current_asm)
                    
                    run_diff(tmp_baseline_file, tmp_current_file, f"snapshot '{canonical_name}'", "CURRENT")
        except KeyboardInterrupt:
            print("\nStopped watch mode.")


def main():
    parser = argparse.ArgumentParser(
        description="Local assembly snapshot and diff tool for quick optimization feedback."
    )
    subparsers = parser.add_subparsers(dest="command", help="Command to execute")

    # save
    p_save = subparsers.add_parser("save", help="Save current assembly state (or Git revision) as a snapshot (default: baseline)")
    p_save.add_argument("name", nargs="?", default="baseline", help="Snapshot name, e.g. 'baseline', 'exp1', or 'git_<ref>' (e.g. 'git_main', 'git_HEAD~1')")

    # diff
    p_diff = subparsers.add_parser("diff", help="Compare current assembly with a snapshot, or two snapshots")
    p_diff.add_argument("target1", nargs="?", default="baseline", help="Snapshot name or git revision (e.g. 'baseline', 'git_main'), or left snapshot")
    p_diff.add_argument("target2", nargs="?", default=None, help="Optional right snapshot name (if comparing two snapshots)")
    p_diff.add_argument("-f", "--function", help="Filter by function name")

    # show
    p_show = subparsers.add_parser("show", help="Show generated assembly for current code or a snapshot")
    p_show.add_argument("name", nargs="?", default=None, help="Optional snapshot name or git revision (e.g. 'git_main')")
    p_show.add_argument("-f", "--function", help="Filter by function name")

    # list
    subparsers.add_parser("list", help="List all saved snapshots")

    # rm
    p_rm = subparsers.add_parser("rm", help="Delete a snapshot")
    p_rm.add_argument("name", help="Name of snapshot to delete (e.g. 'baseline' or 'git_<ref>')")

    # watch
    p_watch = subparsers.add_parser("watch", help="Watch for source file changes and run diff automatically")
    p_watch.add_argument("name", nargs="?", default="baseline", help="Baseline snapshot name or git ref (default: baseline)")
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
