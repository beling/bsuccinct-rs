#!/usr/bin/env python3
"""Build and benchmark historical revisions of the internal ``phast`` program.

The script works in any directory and needs no local checkout: it clones (or
updates) a bare mirror of the BSuccinct repository from GitHub and builds every
selected revision in a temporary Git worktree of that mirror.

All state lives in the working directory (``--work-dir``, the current directory
by default)::

    <work-dir>/.repo/              bare mirror of --repo-url (branch main)
    <work-dir>/.target/            shared CARGO_TARGET_DIR of all builds
    <work-dir>/.worktrees/         temporary worktrees used for builds
    <work-dir>/.lock               guards the cache against concurrent runs
    <work-dir>/failures.log        one line per failed build or run
    <work-dir>/phast_results.csv   default result CSV
    <work-dir>/<date>_<short>/     cached phast binary, logs and metadata

A revision is built with ``cargo build --release --package phast --features
gxhash`` and ``RUSTFLAGS="-C target-cpu=native"``. Dependencies are resolved
from scratch for every build (the repository does not track ``Cargo.lock``);
the resolved lock file is stored in the revision directory for reference.
Cached binaries are reused, so later runs with different parameters only
repeat the benchmark.

The default benchmark is the one used for the PHast+wrap regression history::

    phast -n 100000000 --csv -s8 -b300 -i12 -e5 pluswrap2 2

Every CSV row consists of the revision hash and date followed by the line
printed by ``phast``.

Usage examples::

    python3 phast_revs.py
    python3 phast_revs.py --work-dir revs --build-only
    python3 phast_revs.py --run-only --out other.csv -- \\
        -n 1000000 --csv -s8 -b300 -i1 -e1 pluswrap2 2

Phast arguments must follow ``--``. Without them the default benchmark is
used.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterator, Sequence, TextIO

try:
    import fcntl
except ImportError:  # pragma: no cover - the script is normally used on Unix
    fcntl = None


DEFAULT_SINCE = "2025-07-18"
DEFAULT_REPO_URL = "https://github.com/beling/bsuccinct-rs.git"
DEFAULT_BRANCH = "main"
DEFAULT_PHAST_ARGS: tuple[str, ...] = (
    "-n",
    "100000000",
    "--csv",
    "-s8",
    "-b300",
    "-i12",
    "-e5",
    "pluswrap2",
    "2",
)
BUILD_RUSTFLAGS = "-C target-cpu=native"
BUILD_FEATURES = ("gxhash",)
BUILD_PROFILE = "release"
BUILD_CONFIG = {
    "profile": BUILD_PROFILE,
    "rustflags": BUILD_RUSTFLAGS,
    "features": list(BUILD_FEATURES),
}
CSV_COLUMNS = [
    "revision",
    "date",
    "keys_num",
    "method",
    "k",
    "bits_per_seed",
    "bucket_size100",
    "slice",
    "threads",
    "tries",
    "bits_per_key",
    "bumped_pct",
    "range_overhead_pct",
    "build_ns_per_key",
    "query_ns_per_key",
]


def git(
    ws: Workspace, *args: str, check: bool = True
) -> subprocess.CompletedProcess[str]:
    """Run a Git command in the bare mirror of ``ws``."""
    proc = subprocess.run(
        ["git", "-C", str(ws.mirror), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if check and proc.returncode != 0:
        message = proc.stderr.strip() or proc.stdout.strip()
        raise RuntimeError(f"git {' '.join(args)} failed: {message}")
    return proc


def ensure_mirror(ws: Workspace, repo_url: str, branch: str, fetch: bool) -> None:
    """Clone or update the bare mirror that is the source of all builds.

    A missing mirror is cloned with ``--bare --single-branch --branch``.  An
    existing one is updated in place unless ``fetch`` is false; a failed update
    only produces a warning, so a cached mirror still works offline.  Exits
    when the mirror cannot be created or lacks the requested branch.
    """
    ref = f"refs/heads/{branch}"
    if not ws.mirror.exists():
        print(f"mirror: cloning {repo_url} ({branch}) into {ws.mirror}...")
        proc = subprocess.run(
            [
                "git",
                "clone",
                "--bare",
                "--single-branch",
                "--branch",
                branch,
                repo_url,
                str(ws.mirror),
            ],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if proc.returncode != 0:
            message = proc.stderr.strip() or proc.stdout.strip()
            sys.exit(f"error: cannot clone {repo_url} into {ws.mirror}: {message}")
        state = "cloned"
    else:
        state = "unchanged"
        origin = git(ws, "remote", "get-url", "origin", check=False)
        if origin.returncode == 0 and origin.stdout.strip() != repo_url:
            print(
                f"warning: {ws.mirror} follows {origin.stdout.strip()}, "
                f"not {repo_url}",
                file=sys.stderr,
            )
        if fetch:
            proc = subprocess.run(
                [
                    "git",
                    "-C",
                    str(ws.mirror),
                    "fetch",
                    "--prune",
                    "origin",
                    f"+{ref}:{ref}",
                ],
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
            )
            if proc.returncode != 0:
                message = proc.stderr.strip() or proc.stdout.strip()
                print(
                    f"warning: cannot update {ws.mirror} ({message}); "
                    "using its cached contents",
                    file=sys.stderr,
                )
            else:
                state = "updated"
        if git(ws, "rev-parse", "--verify", ref, check=False).returncode != 0:
            sys.exit(
                f"error: {ws.mirror} has no branch {branch!r}; "
                "use a different --work-dir or remove that directory"
            )
    head = git(ws, "rev-parse", "--short", ref).stdout.strip()
    print(f"mirror: {repo_url} ({branch} @ {head}, {state})")


def command_version(command: str) -> str | None:
    """Return the first line of ``command --version``, or ``None``."""
    try:
        proc = subprocess.run(
            [command, "--version"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except FileNotFoundError:
        return None
    return proc.stdout.strip() if proc.returncode == 0 else None


def find_cargo(explicit: str | None) -> str:
    """Return the ``cargo`` executable to use, or exit with a clear message.

    Without ``explicit`` the candidates are ``$CARGO``, ``cargo`` from ``PATH``,
    ``~/.cargo/bin/cargo`` and ``$CARGO_HOME/bin/cargo``; this makes the script
    work when ``~/.cargo/bin`` is missing from ``PATH``.  A candidate counts
    only if it is executable and answers ``--version``.
    """
    candidates: list[Path] = []
    if explicit:
        candidates.append(Path(explicit).expanduser())
    else:
        env_cargo = os.environ.get("CARGO")
        if env_cargo:
            candidates.append(Path(env_cargo))
        on_path = shutil.which("cargo")
        if on_path:
            candidates.append(Path(on_path))
        try:
            candidates.append(Path.home() / ".cargo" / "bin" / "cargo")
        except RuntimeError:  # pragma: no cover - HOME may be unset
            pass
        cargo_home = os.environ.get("CARGO_HOME")
        if cargo_home:
            candidates.append(Path(cargo_home) / "bin" / "cargo")

    tried: list[str] = []
    for candidate in candidates:
        if str(candidate) in tried:
            continue
        tried.append(str(candidate))
        if not candidate.is_file() or not os.access(candidate, os.X_OK):
            continue
        if command_version(str(candidate)) is not None:
            return str(candidate)
    sys.exit(
        "error: cannot find a working cargo (tried: "
        + (", ".join(tried) if tried else "nothing")
        + "); install Rust or pass --cargo PATH"
    )


def sibling_tool(tool: str, name: str) -> str:
    """Return ``name`` from the directory of ``tool`` if present, else ``name``."""
    candidate = Path(tool).with_name(name)
    return str(candidate) if candidate.is_file() else name


@dataclass(frozen=True)
class Revision:
    """One commit selected for building and benchmarking."""

    full: str
    short: str
    date: str
    timestamp: int
    subject: str


@dataclass(frozen=True)
class Workspace:
    """Cache layout kept in one working directory."""

    root: Path
    """Directory holding the mirror, the build cache and the result files."""

    @property
    def mirror(self) -> Path:
        """Bare Git mirror of the repository, cloned on first use."""
        return self.root / ".repo"

    @property
    def target_dir(self) -> Path:
        """Shared ``CARGO_TARGET_DIR`` of all builds."""
        return self.root / ".target"

    @property
    def worktrees(self) -> Path:
        """Directory with the temporary worktrees used for builds."""
        return self.root / ".worktrees"

    @property
    def lock_file(self) -> Path:
        """File guarding the cache against concurrent script instances."""
        return self.root / ".lock"

    @property
    def failures_log(self) -> Path:
        """Append-only log of failed builds and runs."""
        return self.root / "failures.log"

    def revision_dir(self, rev: Revision) -> Path:
        """Return the cache directory of ``rev`` (``<date>_<short-hash>``)."""
        return self.root / f"{rev.date}_{rev.short}"

    def revision_binary(self, rev: Revision) -> Path:
        """Return the cached ``phast`` executable of ``rev``."""
        return self.revision_dir(rev) / "phast"


def read_revisions(
    ws: Workspace, since: str, branch: str, first_parent: bool
) -> list[Revision]:
    """Return commits reachable from ``branch`` since ``since`` in date order."""
    fmt = "%H%x00%h%x00%cs%x00%ct%x00%s"
    args = ["log", f"--since={since} 00:00:00", f"--format={fmt}"]
    if first_parent:
        args.append("--first-parent")
    args.extend([f"refs/heads/{branch}", "--"])
    output = git(ws, *args).stdout
    revisions: list[Revision] = []
    for line in output.splitlines():
        if not line:
            continue
        parts = line.split("\0")
        if len(parts) != 5:
            raise RuntimeError(f"unexpected git log output: {line!r}")
        full, short, date, timestamp, subject = parts
        revisions.append(Revision(full, short, date, int(timestamp), subject))
    revisions.sort(key=lambda rev: (rev.timestamp, rev.full))
    return revisions


def select_revisions(
    revisions: list[Revision], prefixes: Sequence[str], limit: int | None
) -> list[Revision]:
    """Restrict ``revisions`` to the requested hashes and limit."""
    if prefixes:
        wanted = []
        for value in prefixes:
            wanted.extend(part.strip() for part in value.split(",") if part.strip())
        selected = set()
        for prefix in wanted:
            matches = [
                rev
                for rev in revisions
                if rev.full.startswith(prefix) or rev.short == prefix
            ]
            if not matches:
                sys.exit(f"error: unknown revision: {prefix}")
            if len(matches) > 1:
                hashes = ", ".join(rev.short for rev in matches)
                sys.exit(f"error: ambiguous revision {prefix!r}: {hashes}")
            selected.add(matches[0].full)
        revisions = [rev for rev in revisions if rev.full in selected]
    if limit is not None:
        revisions = revisions[:limit]
    return revisions


def ensure_dirs(ws: Workspace) -> None:
    """Create the cache directories used by the script."""
    ws.target_dir.mkdir(parents=True, exist_ok=True)
    ws.worktrees.mkdir(parents=True, exist_ok=True)


@contextmanager
def exclusive_lock(ws: Workspace) -> Iterator[None]:
    """Prevent two script instances from sharing a working directory."""
    handle = ws.lock_file.open("w", encoding="utf-8")
    try:
        if fcntl is not None:
            try:
                fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                sys.exit(f"error: another instance is already running ({ws.lock_file})")
        handle.write(f"{os.getpid()}\n")
        handle.flush()
        yield
    finally:
        handle.close()


def write_json(path: Path, data: dict) -> None:
    """Write ``data`` to ``path`` atomically."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    os.replace(tmp, path)


def read_json(path: Path) -> dict | None:
    """Read a JSON object, returning ``None`` when it is missing or invalid."""
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None


def remove_worktree(ws: Workspace, path: Path) -> None:
    """Remove a temporary worktree, pruning stale Git metadata."""
    if path.exists():
        subprocess.run(
            ["git", "-C", str(ws.mirror), "worktree", "remove", "--force", str(path)],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if path.exists():
            shutil.rmtree(path, ignore_errors=True)
    git(ws, "worktree", "prune", check=False)


def cleanup_worktrees(ws: Workspace) -> None:
    """Remove worktrees left behind by an interrupted run."""
    ws.worktrees.mkdir(parents=True, exist_ok=True)
    for child in list(ws.worktrees.iterdir()):
        if child.is_dir():
            remove_worktree(ws, child)
        else:
            child.unlink(missing_ok=True)
    git(ws, "worktree", "prune", check=False)


def detect_bucket_format(conf_text: str) -> str:
    """Return ``keys`` for the old bucket-size printing, ``hundreds`` otherwise.

    Before commit 6bd4f6c, ``Conf::print_csv`` printed the ``BucketSize``
    display value (e.g. ``3`` for 300) instead of ``bucket_size100``.
    """
    if re.search(r"Into::<\w+>::into\(self\.bucket_size\(\)\)", conf_text):
        return "hundreds"
    if re.search(r"self\.bucket_size\(\),\s*self\.slice_len", conf_text):
        return "keys"
    return "hundreds"


def revision_bucket_format(ws: Workspace, rev: Revision) -> str:
    """Return the bucket-size format of a revision, using its metadata if possible."""
    meta = read_json(ws.revision_dir(rev) / "meta.json")
    if meta is not None and meta.get("bucket_csv_format") in ("keys", "hundreds"):
        return meta["bucket_csv_format"]
    proc = git(ws, "show", f"{rev.full}:internal/phast/src/conf.rs", check=False)
    if proc.returncode == 0:
        return detect_bucket_format(proc.stdout)
    return "hundreds"


def cached_binary_status(
    ws: Workspace, rev: Revision, force_build: bool
) -> tuple[bool, str]:
    """Return whether the cached binary can be reused and why."""
    binary = ws.revision_binary(rev)
    if not binary.is_file():
        return False, "missing"
    if not os.access(binary, os.X_OK):
        return False, "not executable"
    if force_build:
        return False, "forced rebuild"
    meta = read_json(ws.revision_dir(rev) / "meta.json")
    if meta is None:
        return True, "no metadata"
    if meta.get("revision") != rev.full:
        return False, "metadata revision mismatch"
    if meta.get("status") != "ok":
        return False, "last build failed"
    if meta.get("build_config") != BUILD_CONFIG:
        return False, "build configuration changed"
    return True, "cached"


def build_revision(
    ws: Workspace, rev: Revision, keep_worktree: bool, cargo: str
) -> tuple[bool, float, Path, str | None]:
    """Build one revision and cache its ``phast`` executable.

    Returns ``(success, seconds, build_log, error)``.  The build runs in a
    temporary worktree so the mirror stays untouched and every revision is
    built from its own sources.  Dependencies are resolved from scratch; the
    resulting ``Cargo.lock`` is copied next to the binary for reference.
    """
    cache_dir = ws.revision_dir(rev)
    binary = ws.revision_binary(rev)
    cache_dir.mkdir(parents=True, exist_ok=True)
    build_log = cache_dir / "build.log"
    worktree = ws.worktrees / rev.full
    started = time.monotonic()
    started_at = datetime.now(timezone.utc)
    bucket_format = "hundreds"
    command = [
        cargo,
        "build",
        "--release",
        "--package",
        "phast",
        "--features",
        ",".join(BUILD_FEATURES),
        "--manifest-path",
        str(worktree / "Cargo.toml"),
    ]
    env = os.environ.copy()
    env["RUSTFLAGS"] = BUILD_RUSTFLAGS
    env["CARGO_TARGET_DIR"] = str(ws.target_dir)
    env["CARGO_TERM_COLOR"] = "never"

    log_created = False
    remove_worktree(ws, worktree)
    try:
        git(ws, "worktree", "add", "--detach", str(worktree), rev.full)
        conf_path = worktree / "internal/phast/src/conf.rs"
        if conf_path.is_file():
            bucket_format = detect_bucket_format(
                conf_path.read_text(encoding="utf-8", errors="replace")
            )
        stale_binary = ws.target_dir / "release/phast"
        stale_binary.unlink(missing_ok=True)
        with build_log.open("w", encoding="utf-8") as log:
            log_created = True
            log.write(f"# revision: {rev.full} ({rev.short}) {rev.date} {rev.subject}\n")
            log.write(f"# command: {' '.join(command)}\n")
            log.write(f"# RUSTFLAGS: {BUILD_RUSTFLAGS}\n")
            log.write(f"# CARGO_TARGET_DIR: {ws.target_dir}\n")
            log.write(f"# started: {started_at.isoformat()}\n\n")
            log.flush()
            proc = subprocess.run(
                command,
                cwd=str(worktree),
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                text=True,
            )
            if proc.returncode != 0:
                log.write(f"\n# failed with exit code {proc.returncode}\n")
                raise RuntimeError(
                    f"cargo build failed with exit code {proc.returncode}; see {build_log}"
                )
            log.write("\n# finished successfully\n")

        built = ws.target_dir / "release/phast"
        if not built.is_file():
            raise RuntimeError(
                f"cargo reported success but {built} is missing; see {build_log}"
            )
        tmp = cache_dir / ".phast.tmp"
        shutil.copy2(built, tmp)
        tmp.chmod(0o755)
        os.replace(tmp, binary)
        worktree_lock = worktree / "Cargo.lock"
        if worktree_lock.is_file():
            shutil.copy2(worktree_lock, cache_dir / "Cargo.lock")

        elapsed = time.monotonic() - started
        meta = {
            "revision": rev.full,
            "short": rev.short,
            "date": rev.date,
            "subject": rev.subject,
            "status": "ok",
            "built_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "build_seconds": round(elapsed, 3),
            "rustc": command_version(sibling_tool(cargo, "rustc")),
            "cargo": command_version(cargo),
            "build_config": BUILD_CONFIG,
            "bucket_csv_format": bucket_format,
            "binary_size": binary.stat().st_size,
            "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        }
        write_json(cache_dir / "meta.json", meta)
        return True, elapsed, build_log, None
    except Exception as exc:  # noqa: BLE001 - failures are recorded per revision
        elapsed = time.monotonic() - started
        error = f"{type(exc).__name__}: {exc}"
        if log_created:
            with build_log.open("a", encoding="utf-8") as log:
                log.write(f"\n# error: {error}\n")
        else:
            build_log.write_text(
                f"# revision: {rev.full} ({rev.short}) {rev.date} {rev.subject}\n"
                f"# started: {started_at.isoformat()}\n"
                f"# error before cargo started: {error}\n",
                encoding="utf-8",
            )
        meta = {
            "revision": rev.full,
            "short": rev.short,
            "date": rev.date,
            "subject": rev.subject,
            "status": "failed",
            "built_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "build_seconds": round(elapsed, 3),
            "rustc": command_version(sibling_tool(cargo, "rustc")),
            "cargo": command_version(cargo),
            "build_config": BUILD_CONFIG,
            "bucket_csv_format": bucket_format,
            "error": error,
        }
        write_json(cache_dir / "meta.json", meta)
        return False, elapsed, build_log, error
    finally:
        try:
            if keep_worktree:
                print(f"  kept worktree: {worktree}", file=sys.stderr)
            else:
                remove_worktree(ws, worktree)
        except Exception as cleanup_error:  # noqa: BLE001 - do not mask build result
            print(f"  warning: cannot remove {worktree}: {cleanup_error}", file=sys.stderr)


def args_hash(args: Sequence[str]) -> str:
    """Return a short stable identifier for a phast argument list."""
    return hashlib.sha1("\0".join(args).encode("utf-8")).hexdigest()[:8]


def option_value(args: Sequence[str], short: str, long: str) -> str | None:
    """Return the value of a short or long option from a phast argument list."""
    for index, arg in enumerate(args):
        if arg in (short, long):
            return args[index + 1] if index + 1 < len(args) else None
        if arg.startswith(long + "="):
            return arg.split("=", 1)[1]
        if arg.startswith(short) and arg != short:
            return arg[len(short) :]
    return None


def extract_fields(
    stdout: str, bucket_format: str, expected_n: str | None
) -> list[str]:
    """Extract and normalize the 13 phast CSV fields from its stdout.

    Old revisions print the bucket size as a fraction of 100 (``3`` instead of
    ``300``) and the turbo bucket as ``4 (turbo)``; both forms are normalized.
    """
    data_lines = []
    for raw_line in stdout.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        parts = line.split()
        if parts and parts[0].isdigit():
            data_lines.append(line)
    if not data_lines:
        raise ValueError("phast produced no CSV data line")
    if len(data_lines) > 1:
        print(
            f"  warning: phast printed {len(data_lines)} data lines; using the last",
            file=sys.stderr,
        )

    parts = data_lines[-1].split()
    turbo = False
    if "(turbo)" in parts:
        index = parts.index("(turbo)")
        if index == 0 or parts[index - 1] != "4":
            raise ValueError("cannot normalize the turbo bucket-size field")
        parts[index - 1 : index + 1] = ["400"]
        turbo = True
    expected_fields = len(CSV_COLUMNS) - 2
    if len(parts) != expected_fields:
        raise ValueError(
            f"expected {expected_fields} phast fields, got {len(parts)}: "
            f"{data_lines[-1]!r}"
        )
    if bucket_format == "keys" and not turbo:
        try:
            bucket = float(parts[4])
        except ValueError as exc:
            raise ValueError(f"invalid bucket_size100 field {parts[4]!r}") from exc
        bucket *= 100.0
        parts[4] = str(int(bucket)) if bucket.is_integer() else f"{bucket:g}"
    if expected_n is not None and parts[0] != expected_n:
        raise ValueError(f"expected keys_num {expected_n}, got {parts[0]}")
    return parts


def run_revision(
    ws: Workspace,
    rev: Revision,
    phast_args: Sequence[str],
    timeout: int,
    bucket_format: str,
    expected_n: str | None,
) -> tuple[bool, list[str] | None, str | None]:
    """Run the cached binary and return its normalized CSV fields."""
    cache_dir = ws.revision_dir(rev)
    binary = ws.revision_binary(rev)
    log_path = cache_dir / f"run_{args_hash(phast_args)}.log"
    command = [str(binary), *phast_args]
    started_at = datetime.now(timezone.utc)
    proc: subprocess.CompletedProcess[str] | None = None
    try:
        proc = subprocess.run(
            command,
            cwd=str(cache_dir),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=None if timeout <= 0 else timeout,
        )
    except subprocess.TimeoutExpired as exc:
        stdout = exc.stdout or ""
        stderr = exc.stderr or ""
        if isinstance(stdout, bytes):
            stdout = stdout.decode("utf-8", errors="replace")
        if isinstance(stderr, bytes):
            stderr = stderr.decode("utf-8", errors="replace")
        with log_path.open("w", encoding="utf-8") as log:
            log.write(f"# revision: {rev.full} ({rev.short}) {rev.date}\n")
            log.write(f"# command: {' '.join(command)}\n")
            log.write(f"# started: {started_at.isoformat()}\n")
            log.write(f"# timeout: {timeout}s\n")
            log.write(f"# stdout:\n{stdout}\n# stderr:\n{stderr}\n")
        return False, None, f"phast timed out after {timeout}s; see {log_path}"
    except OSError as exc:
        return False, None, f"cannot execute {binary}: {exc}"

    assert proc is not None
    with log_path.open("w", encoding="utf-8") as log:
        log.write(f"# revision: {rev.full} ({rev.short}) {rev.date}\n")
        log.write(f"# command: {' '.join(command)}\n")
        log.write(f"# started: {started_at.isoformat()}\n")
        log.write(f"# exit code: {proc.returncode}\n")
        log.write(f"# stdout:\n{proc.stdout}\n# stderr:\n{proc.stderr}\n")
    if proc.returncode != 0:
        return (
            False,
            None,
            f"phast exited with code {proc.returncode}; see {log_path}",
        )
    try:
        fields = extract_fields(proc.stdout, bucket_format, expected_n)
    except ValueError as exc:
        return False, None, f"{exc}; see {log_path}"
    return True, fields, None


def prepare_sidecar(out_path: Path, phast_args: Sequence[str]) -> None:
    """Check or create the ``.args`` sidecar guarding the output CSV."""
    sidecar = out_path.with_name(out_path.name + ".args")
    payload = {
        "argv": list(phast_args),
        "created": datetime.now(timezone.utc).isoformat(timespec="seconds"),
    }
    if sidecar.exists():
        existing = read_json(sidecar)
        if existing is None:
            sys.exit(f"error: cannot parse {sidecar}")
        if existing.get("argv") != list(phast_args):
            sys.exit(
                f"error: {out_path} was created with different phast arguments; "
                "use a different --out"
            )
    else:
        if out_path.exists() and out_path.stat().st_size > 0:
            print(
                f"warning: {out_path} has no .args sidecar; adopting current arguments",
                file=sys.stderr,
            )
        write_json(sidecar, payload)


def rewrite_csv(path: Path, rows: list[list[str]]) -> None:
    """Atomically rewrite a result CSV with the standard header and ``rows``."""
    tmp = path.with_name(path.name + ".tmp")
    with tmp.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(CSV_COLUMNS)
        writer.writerows(rows)
    os.replace(tmp, path)


def prepare_csv(
    out_path: Path,
    phast_args: Sequence[str],
    selected_shorts: set[str],
    force_run: bool,
) -> tuple[TextIO, Any, set[str]]:
    """Open the output CSV for appending and return existing revision IDs."""
    prepare_sidecar(out_path, phast_args)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    has_content = out_path.exists() and out_path.stat().st_size > 0
    rows: list[list[str]] = []
    if has_content:
        with out_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.reader(handle)
            header = next(reader, None)
            if header != CSV_COLUMNS:
                sys.exit(
                    f"error: {out_path} has an unexpected CSV header; expected "
                    f"{','.join(CSV_COLUMNS)}"
                )
            rows = [row for row in reader if row]
        if force_run:
            rows = [row for row in rows if row[0] not in selected_shorts]
            rewrite_csv(out_path, rows)
    existing = {row[0] for row in rows if row}
    handle = out_path.open("a", newline="", encoding="utf-8")
    writer = csv.writer(handle, lineterminator="\n")
    if not has_content:
        writer.writerow(CSV_COLUMNS)
        handle.flush()
    return handle, writer, existing


def record_failure(
    ws: Workspace, rev: Revision, stage: str, message: str, log_path: Path
) -> None:
    """Append a failure record to the persistent failure log."""
    timestamp = datetime.now(timezone.utc).isoformat(timespec="seconds")
    with ws.failures_log.open("a", encoding="utf-8") as handle:
        handle.write(
            f"{timestamp}\t{rev.short}\t{rev.date}\t{stage}\t{message}\t{log_path}\n"
        )


def split_argv(argv: Sequence[str]) -> tuple[list[str], list[str]]:
    """Split script arguments from the phast arguments following ``--``."""
    values = list(argv)
    if "--" in values:
        index = values.index("--")
        return values[:index], values[index + 1 :]
    return values, []


def validate_since(value: str) -> None:
    """Validate the ``--since`` date format."""
    try:
        datetime.strptime(value, "%Y-%m-%d")
    except ValueError:
        sys.exit("error: --since must have the YYYY-MM-DD form")


def resolve_out(ws: Workspace, value: str | None) -> Path:
    """Return the absolute CSV path (default: ``<work-dir>/phast_results.csv``)."""
    if value is None:
        return ws.root / "phast_results.csv"
    path = Path(value).expanduser()
    if not path.is_absolute():
        path = (Path.cwd() / path).resolve()
    return path


def build_parser() -> argparse.ArgumentParser:
    """Create the command-line parser."""
    default_args = " ".join(DEFAULT_PHAST_ARGS)
    parser = argparse.ArgumentParser(
        description="Build and benchmark historical revisions of internal/phast.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Phast arguments must follow '--'.\n"
            f"Default benchmark: {default_args}\n\n"
            "The repository is cloned from --repo-url into <work-dir>/.repo and\n"
            "every revision is built in a temporary worktree of that mirror.\n\n"
            "Examples:\n"
            "  python3 phast_revs.py\n"
            "  python3 phast_revs.py --work-dir revs --build-only\n"
            "  python3 phast_revs.py --run-only --out other.csv -- "
            "-n 1000000 --csv -s8 -b300 -i1 -e1 pluswrap2 2\n"
        ),
    )
    parser.add_argument(
        "--work-dir",
        default=".",
        metavar="DIR",
        help="directory for the mirror, cache and results (default: current directory)",
    )
    parser.add_argument(
        "--repo-url",
        default=DEFAULT_REPO_URL,
        metavar="URL",
        help=f"repository to clone (default: {DEFAULT_REPO_URL})",
    )
    parser.add_argument(
        "--branch",
        default=DEFAULT_BRANCH,
        metavar="NAME",
        help=f"branch whose history is benchmarked (default: {DEFAULT_BRANCH})",
    )
    parser.add_argument(
        "--no-fetch",
        action="store_true",
        help="use the cached mirror without updating it",
    )
    parser.add_argument(
        "--cargo",
        metavar="PATH",
        help="cargo executable (default: found via PATH, ~/.cargo/bin or CARGO_HOME)",
    )
    parser.add_argument(
        "--since",
        default=DEFAULT_SINCE,
        metavar="YYYY-MM-DD",
        help=f"include commits since this date (default: {DEFAULT_SINCE})",
    )
    parser.add_argument(
        "--out",
        metavar="FILE",
        help="result CSV (default: <work-dir>/phast_results.csv)",
    )
    parser.add_argument(
        "--rev",
        action="append",
        default=[],
        metavar="REV",
        help="restrict to a revision (hash or unique prefix; repeatable)",
    )
    parser.add_argument(
        "--limit",
        type=int,
        metavar="N",
        help="process at most N revisions after filtering",
    )
    parser.add_argument(
        "--first-parent",
        action="store_true",
        help="use only the first-parent history of main",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--build-only",
        action="store_true",
        help="build missing binaries without running the benchmark",
    )
    mode.add_argument(
        "--run-only",
        "--no-build",
        dest="run_only",
        action="store_true",
        help="run cached binaries only; never build",
    )
    parser.add_argument(
        "--force-build",
        action="store_true",
        help="rebuild even when a cached binary exists",
    )
    parser.add_argument(
        "--force-run",
        action="store_true",
        help="rerun revisions already present in the output CSV",
    )
    parser.add_argument(
        "--keep-worktree",
        action="store_true",
        help="keep temporary worktrees for debugging",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=3600,
        metavar="SECONDS",
        help="timeout for one phast run, 0 disables it (default: 3600)",
    )
    return parser


def main(argv: Sequence[str]) -> int:
    """Run the requested build/benchmark workflow."""
    script_argv, phast_argv = split_argv(argv)
    parser = build_parser()
    args = parser.parse_args(script_argv)

    validate_since(args.since)
    if args.limit is not None and args.limit < 0:
        parser.error("--limit must be non-negative")
    if args.timeout < 0:
        parser.error("--timeout must be non-negative")
    if args.run_only and args.force_build:
        parser.error("--force-build cannot be used with --run-only")
    if args.build_only and args.force_run:
        parser.error("--force-run cannot be used with --build-only")

    run_args = tuple(phast_argv) if phast_argv else DEFAULT_PHAST_ARGS
    if "--csv" not in run_args:
        parser.error("custom phast arguments must include --csv")

    work_dir = Path(args.work_dir).expanduser()
    if not work_dir.is_absolute():
        work_dir = Path.cwd() / work_dir
    ws = Workspace(work_dir.resolve())
    out_path = resolve_out(ws, args.out)
    if command_version("git") is None:
        sys.exit("error: git is required but was not found in PATH")
    cargo: str | None = None if args.run_only else find_cargo(args.cargo)

    ws.root.mkdir(parents=True, exist_ok=True)
    with exclusive_lock(ws):
        ensure_dirs(ws)
        ensure_mirror(ws, args.repo_url, args.branch, not args.no_fetch)
        cleanup_worktrees(ws)
        revisions = read_revisions(ws, args.since, args.branch, args.first_parent)
        revisions = select_revisions(revisions, args.rev, args.limit)
        if not revisions:
            print("No revisions selected.")
            return 0
        print(
            f"Selected {len(revisions)} revisions from {revisions[0].date} "
            f"to {revisions[-1].date}."
        )

        selected_shorts = {rev.short for rev in revisions}
        csv_handle = None
        csv_writer = None
        existing: set[str] = set()
        if not args.build_only:
            csv_handle, csv_writer, existing = prepare_csv(
                out_path, run_args, selected_shorts, args.force_run
            )

        counts = {
            "built": 0,
            "cached": 0,
            "build_failed": 0,
            "run_ok": 0,
            "run_skipped": 0,
            "run_failed": 0,
        }
        try:
            for index, rev in enumerate(revisions, 1):
                prefix = f"[{index}/{len(revisions)}] {rev.short} {rev.date}"
                if not args.run_only:
                    ok, reason = cached_binary_status(ws, rev, args.force_build)
                    if ok:
                        print(f"{prefix} cached ({reason})")
                        counts["cached"] += 1
                    else:
                        print(f"{prefix} building ({reason})...")
                        assert cargo is not None
                        ok, seconds, build_log, error = build_revision(
                            ws, rev, args.keep_worktree, cargo
                        )
                        if ok:
                            print(f"{prefix} built in {seconds:.1f}s")
                            counts["built"] += 1
                        else:
                            print(f"{prefix} BUILD FAILED: {error}")
                            counts["build_failed"] += 1
                            record_failure(
                                ws, rev, "build", error or "unknown", build_log
                            )
                            continue
                else:
                    binary = ws.revision_binary(rev)
                    if not binary.is_file():
                        error = f"missing cached binary {binary}"
                        print(f"{prefix} RUN FAILED: {error}")
                        counts["run_failed"] += 1
                        record_failure(ws, rev, "run", error, ws.revision_dir(rev))
                        continue

                if args.build_only:
                    continue
                if rev.short in existing and not args.force_run:
                    print(f"{prefix} already in {out_path.name}; skipping run")
                    counts["run_skipped"] += 1
                    continue

                bucket_format = revision_bucket_format(ws, rev)
                expected_n = option_value(run_args, "-n", "--keys-num")
                ok, fields, error = run_revision(
                    ws, rev, run_args, args.timeout, bucket_format, expected_n
                )
                if ok and csv_writer is not None and csv_handle is not None:
                    assert fields is not None
                    csv_writer.writerow([rev.short, rev.date, *fields])
                    csv_handle.flush()
                    existing.add(rev.short)
                    counts["run_ok"] += 1
                    print(f"{prefix} run ok")
                    print(prefix, *fields)
                else:
                    print(f"{prefix} RUN FAILED: {error}")
                    counts["run_failed"] += 1
                    record_failure(
                        ws, rev, "run", error or "unknown", ws.revision_dir(rev)
                    )
        finally:
            if csv_handle is not None:
                csv_handle.close()

        print()
        print(f"Revisions selected: {len(revisions)}")
        print(
            f"Builds: {counts['built']} built, {counts['cached']} cached, "
            f"{counts['build_failed']} failed"
        )
        if not args.build_only:
            print(
                f"Runs: {counts['run_ok']} ok, {counts['run_skipped']} skipped, "
                f"{counts['run_failed']} failed"
            )
            print(f"CSV: {out_path}")
        if counts["build_failed"] or counts["run_failed"]:
            print(f"Failures logged in {ws.failures_log}")
            return 1
        return 0


def entrypoint() -> None:
    """Run ``main`` and translate interrupts into the usual exit code."""
    try:
        code = main(sys.argv[1:])
    except KeyboardInterrupt:
        print("\nInterrupted.", file=sys.stderr)
        sys.exit(130)
    sys.exit(code)


if __name__ == "__main__":
    entrypoint()



