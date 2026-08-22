#!/usr/bin/env python3
"""
Local assembly snapshot management and diff tool for `asmview`.
Allows saving snapshots, comparing current/named versions, showing assembly,
watching for live changes, and fetching/caching snapshots from Git revisions (git_<ref>).
Supports target:filter syntax (e.g. baseline:fn, :fn, git_main:fn).
"""

import argparse
import re
import shlex
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
    combined_output = (res.stdout or "") + "\n" + (res.stderr or "")
    for line in combined_output.splitlines():
        match = pattern.search(line.strip())
        if match:
            functions.append(match.group(1))

    if not functions:
        lib_rs = cwd / "internal" / "asmview" / "src" / "lib.rs"
        if lib_rs.exists():
            fn_pattern = re.compile(r'pub\s+extern\s+"C"\s+fn\s+([a-zA-Z0-9_]+)')
            functions = fn_pattern.findall(lib_rs.read_text())

    return sorted(set(functions))


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
    for fn in all_functions:
        out.append(f"=== FUNCTION: {fn} ===")
        asm = get_function_asm(cwd, fn)
        out.append(asm)
        out.append("\n")
    return "\n".join(out)


def filter_asm(asm_text: str, filter_pattern: str | None = None, target_label: str = "") -> str:
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

    if not filtered_blocks and asm_text.strip():
        label = f" in '{target_label}'" if target_label else ""
        print(f"Warning: No functions matching filter '{filter_pattern}' found{label}.", file=sys.stderr)

    return "\n".join(filtered_blocks)


def parse_target_filter(spec: str | None, default_target: str = "baseline") -> tuple[str, str | None]:
    """
    Parses strings in format '[target][:filter]'.
    Examples:
      None -> (default_target, None)
      "" -> (default_target, None)
      ":fn" -> (default_target, "fn")
      "snap" -> ("snap", None)
      "snap:fn" -> ("snap", "fn")
    """
    if not spec:
        return default_target, None

    if ":" in spec:
        target_part, _, filter_part = spec.partition(":")
        target = target_part if target_part else default_target
        filter_pat = filter_part if filter_part else None
        return target, filter_pat
    else:
        return spec, None


def resolve_git_ref(workspace_root: Path, ref: str) -> str:
    """Resolve a git reference (branch, tag, relative ref like HEAD~1) to a short commit hash."""
    cmd = ["git", "rev-parse", "--short=10", "--verify", ref]
    res = subprocess.run(cmd, cwd=workspace_root, capture_output=True, text=True)
    if res.returncode != 0:
        print(f"Error: Git revision '{ref}' not found:\n{res.stderr.strip()}", file=sys.stderr)
        sys.exit(1)
    return res.stdout.strip()


def get_snapshot_file(workspace_root: Path, name: str) -> tuple[str, Path]:
    """
    Resolves the canonical name and path to .snapshots/<name>.asm without creating it.
    """
    snapshots_dir = get_snapshots_dir(workspace_root)
    if name.startswith("git_"):
        raw_ref = name[len("git_"):]
        try:
            commit_hash = resolve_git_ref(workspace_root, raw_ref)
            name = f"git_{commit_hash}"
        except SystemExit:
            pass
    return name, snapshots_dir / f"{name}.asm"


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


def get_target_asm(workspace_root: Path, target_name: str, auto_create_baseline: bool = False) -> tuple[str, str]:
    """
    Retrieves the assembly content for a given target name ("CURRENT", a snapshot, or "git_<ref>").
    Returns (label_name, asm_content). Exits if snapshot does not exist (unless auto_create_baseline is True for 'baseline').
    """
    if target_name == "CURRENT":
        return "current", get_all_asm(workspace_root)

    side_name, target_file = ensure_git_snapshot(workspace_root, target_name)
    if not target_file.exists():
        if auto_create_baseline and target_name == "baseline":
            print(f"Baseline snapshot '{baseline_name}' not found. Generating and saving initial snapshot...")
            asm = get_all_asm(workspace_root)
            target_file.write_text(asm)
            print(f"Saved snapshot 'baseline'.")
            return side_name, asm
        print(f"Error: Snapshot '{target_name}' does not exist.", file=sys.stderr)
        sys.exit(1)
    return side_name, target_file.read_text()


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
    target, filter_pat = parse_target_filter(args.target, default_target="CURRENT")
    side_name, asm_content = get_target_asm(workspace_root, target)
    print(filter_asm(asm_content, filter_pat, side_name))


def cmd_list(args, workspace_root: Path):
    functions = list_asmview_functions(workspace_root)
    if functions:
        print(f"Available functions in asmview ({len(functions)}):")
        for fn in functions:
            print(f"  - {fn}")
        print()

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
    name, target_file = get_snapshot_file(workspace_root, args.name)
    if not target_file.exists():
        print(f"Snapshot '{name}' does not exist, nothing to remove.")
        return

    target_file.unlink()
    print(f"Deleted snapshot '{name}'.")


def run_diff(left_file: Path, right_file: Path, left_label: str, right_label: str, tool: str | None = None):
    print("\n" + "=" * 80)
    print(f"DIFF: {left_label} (left) vs {right_label} (right)")
    print("=" * 80 + "\n")

    if tool:
        tool_cmd = shlex.split(tool) + [str(left_file), str(right_file)]
        res = subprocess.run(tool_cmd)
        if res.returncode == 0:
            print("Diff tool finished.")
    else:
        cmd = ["git", "diff", "--no-index", "--color=always", str(left_file), str(right_file)]
        res = subprocess.run(cmd, capture_output=True, text=True)

        if res.returncode == 0:
            print("No assembly differences found!")
        else:
            print(res.stdout)


def cmd_diff(args, workspace_root: Path):
    # Parse positional arguments:
    # 0 items: [] -> baseline vs CURRENT (no filter)
    # 1 item:  [":fn"] -> baseline:fn vs CURRENT:fn
    #          ["snap:fn"] -> snap:fn vs CURRENT:fn
    # 2 items: [":fn1", ":fn2"] -> CURRENT:fn1 vs CURRENT:fn2
    #          ["snap1", "snap2"] -> snap1 vs snap2
    #          ["snap1:fn1", "snap2:fn2"] -> snap1:fn1 vs snap2:fn2
    #          ["snap", ":fn"] -> snap:fn vs CURRENT:fn
    # 3 items: ["snap1", "snap2", ":fn"] -> snap1:fn vs snap2:fn
    targets = args.targets or []
    global_filter = None

    if len(targets) == 3:
        target1_spec, target2_spec, filter_spec = targets
        _, global_filter = parse_target_filter(filter_spec, default_target="")
    elif len(targets) == 2:
        if targets[0].startswith(":") and targets[1].startswith(":"):
            target1_spec, target2_spec = targets[0], targets[1]
        elif not targets[0].startswith(":") and targets[1].startswith(":"):
            target1_spec = targets[0]
            target2_spec = "CURRENT"
            _, global_filter = parse_target_filter(targets[1], default_target="")
        else:
            target1_spec, target2_spec = targets[0], targets[1]
    elif len(targets) == 1:
        if targets[0].startswith(":"):
            target1_spec = "baseline"
            target2_spec = "CURRENT"
            _, global_filter = parse_target_filter(targets[0], default_target="")
        else:
            t_name, t_filter = parse_target_filter(targets[0], default_target="baseline")
            target1_spec = t_name
            target2_spec = "CURRENT"
            global_filter = t_filter
    else:
        target1_spec = "baseline"
        target2_spec = "CURRENT"

    target1_name, filter1 = parse_target_filter(target1_spec, default_target="baseline" if target2_spec == "CURRENT" else "CURRENT")
    target2_name, filter2 = parse_target_filter(target2_spec, default_target="CURRENT")

    if global_filter:
        filter1 = filter1 or global_filter
        filter2 = filter2 or global_filter

    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir)

        # Allow auto-creation only if left side is default 'baseline' and right side is 'CURRENT'
        auto_baseline = (target1_name == "baseline" and target2_name == "CURRENT")
        side1_name, side1_asm = get_target_asm(workspace_root, target1_name, auto_create_baseline=auto_baseline)
        side2_name, side2_asm = get_target_asm(workspace_root, target2_name)

        asm1 = filter_asm(side1_asm, filter1, side1_name)
        asm2 = filter_asm(side2_asm, filter2, side2_name)

        fn1_tag = f".{filter1}" if filter1 else ""
        fn2_tag = f".{filter2}" if filter2 else ""

        tmp_file1 = tmp_path / f"{side1_name}{fn1_tag}.asm"
        tmp_file2 = tmp_path / f"{side2_name}{fn2_tag}.asm"
        tmp_file1.write_text(asm1)
        tmp_file2.write_text(asm2)

        label1 = f"'{side1_name}{fn1_tag}'"
        label2 = f"'{side2_name}{fn2_tag}'"
        run_diff(tmp_file1, tmp_file2, label1, label2, tool=args.tool)


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
    baseline_name, filter_pat = parse_target_filter(args.target, default_target="baseline")
    canonical_name, baseline_asm_full = get_target_asm(workspace_root, baseline_name, auto_create_baseline=True)

    fn_tag = f".{filter_pat}" if filter_pat else ""
    print(f"\n[Watch Mode] Watching for changes in Rust source files (*.rs)...")
    print(f"Comparing against baseline: '{canonical_name}{fn_tag}'")
    if filter_pat:
        print(f"Filtering function: '{filter_pat}'")
    print("Press Ctrl+C to exit.\n")

    last_files = get_watched_files(workspace_root)

    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp_path = Path(tmp_dir)
        tmp_baseline_file = tmp_path / f"{canonical_name}{fn_tag}.asm"
        tmp_current_file = tmp_path / f"current{fn_tag}.asm"

        try:
            while True:
                time.sleep(1.0)
                current_files = get_watched_files(workspace_root)
                changed = [p for p, mtime in current_files.items() if last_files.get(p) != mtime]

                if changed:
                    last_files = current_files
                    print(f"\n[{datetime.now().strftime('%H:%M:%S')}] Detected change in: {', '.join(p.name for p in changed[:3])}...")

                    # Refresh baseline content (with filter)
                    baseline_asm = filter_asm(baseline_asm_full, filter_pat, canonical_name)
                    tmp_baseline_file.write_text(baseline_asm)

                    # Get and filter current assembly
                    current_asm = filter_asm(get_all_asm(workspace_root), filter_pat, "current")
                    tmp_current_file.write_text(current_asm)

                    run_diff(tmp_baseline_file, tmp_current_file, f"snapshot '{canonical_name}{fn_tag}'", f"CURRENT{fn_tag}")
        except KeyboardInterrupt:
            print("\nStopped watch mode.")


def main():
    parser = argparse.ArgumentParser(
        description="Local assembly snapshot and diff tool with target:filter syntax."
    )
    subparsers = parser.add_subparsers(dest="command", help="Command to execute")

    # save
    p_save = subparsers.add_parser("save", help="Save current assembly state (or Git revision) as a snapshot (default: baseline)")
    p_save.add_argument("name", nargs="?", default="baseline", help="Snapshot name, e.g. 'baseline', 'exp1', or 'git_<ref>' (e.g. 'git_main', 'git_HEAD~1')")

    # diff
    p_diff = subparsers.add_parser("diff", help="Compare current assembly with a snapshot, or two snapshots/functions")
    p_diff.add_argument("targets", nargs="*", help="Targets to compare: [snap1[:fn1]] [snap2[:fn2]] [:common_fn] or :fn1 :fn2")
    p_diff.add_argument("-t", "--tool", help="Custom diff tool program to run (e.g. 'meld', 'kdiff3', 'diff -u')")

    # show
    p_show = subparsers.add_parser("show", help="Show generated assembly for current code or a snapshot")
    p_show.add_argument("target", nargs="?", default=None, help="Optional snapshot and/or function filter, e.g. ':fn', 'baseline', 'git_main:fn'")

    # list
    subparsers.add_parser("list", help="List available functions and all saved snapshots")

    # rm
    p_rm = subparsers.add_parser("rm", help="Delete a snapshot")
    p_rm.add_argument("name", help="Name of snapshot to delete (e.g. 'baseline' or 'git_<ref>')")

    # watch
    p_watch = subparsers.add_parser("watch", help="Watch for source file changes and run diff automatically")
    p_watch.add_argument("target", nargs="?", default="baseline", help="Baseline snapshot and/or function filter (e.g. 'baseline', ':fn', 'git_main:fn')")

    args = parser.parse_args()
    workspace_root = get_workspace_root()

    if not args.command:
        args.targets = []
        args.tool = None
        cmd_diff(args, workspace_root)
    elif args.command == "diff": cmd_diff(args, workspace_root)
    elif args.command == "save": cmd_save(args, workspace_root)
    elif args.command == "show": cmd_show(args, workspace_root)
    elif args.command == "list": cmd_list(args, workspace_root)
    elif args.command == "rm": cmd_rm(args, workspace_root)
    elif args.command == "watch": cmd_watch(args, workspace_root)


if __name__ == "__main__":
    main()
