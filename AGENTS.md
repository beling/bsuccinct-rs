# AGENTS.md

Guidance for AI agents working in this repository (BSuccinct: Rust libraries and programs focused on succinct data structures).

## Sources of truth

- The root `README.md` is the catalogue of published crates (libraries and programs): names, crates.io/docs.rs links, one-line descriptions. It is the only place where the list lives — do not duplicate it, and keep it up to date when a crate is added, renamed, or removed.
- Each crate has its own `README.md` and it *is* the crate-level documentation: `src/lib.rs`/`src/main.rs` starts with `#![doc = include_str!("../README.md")]`. Crate-specific facts (features, usage examples, bibliography, instructions for reproducing published experiments) belong there, not in this file.
- `internal/` holds unpublished crates that only support development (for example `asmview`, `phast`).
- Everything in this file applies to the whole workspace; crate `README.md`s are the place for details.

## Language and Markdown style

- Everything is written in English: this file, all `README.md` files, documentation, code comments and commit messages.
- Lines in Markdown files are never hard-wrapped (no length limit): one paragraph is one line. Break lines only for logical reasons (a new thought, a list item, a table row, etc.).
- Be concise and concrete; no filler and no marketing tone. Keep the established shape: the first line of a crate README defines the crate, e.g. `` `bitm` is a Rust library by Piotr Beling for bit and bitmap (bit vector) manipulation. ``
- Keep the notation used across the docs (for example *O(n)*, *n*-element set, *i*-th level).

## Layout and workspace

- One crate per top-level directory; every crate is a workspace member declared in the root `Cargo.toml` (`internal/*` covers the internal ones). Do not add a crate outside the workspace.
- Helper scripts (Python/bash) live next to what they support, e.g. `internal/asmview/asm.py`, `csf_benchmark/utils/csf_benchmark_plot.py`, `cseq_benchmark/utils/cseq_benchmark_tabs.py`.
- `.gitignore` is the source of truth for what is not tracked (for example `Cargo.lock` is ignored intentionally). Never commit ignored files, and never remove or relax ignore entries without the user's consent.

## Building, testing, verifying

- `cargo build` and `cargo test` must pass; the CI (`.github/workflows/rust.yml`) runs exactly these two commands with `RUSTFLAGS="-C target-cpu=native"` (the same flag is already set in `.cargo/config.toml`).
- Documentation examples are real tests: because a crate README is included as crate docs, Rust code blocks in READMEs and in `///` comments are compiled and executed by `cargo test`. Keep them correct and runnable.
- Feature-gated code must be checked with the relevant feature sets (for example `--no-default-features`, `--features gxhash`, `fmph`, `fmph-key-access`). Most features are delegated to `seedable_hash`/`ph`, so a change may affect several crates.
- Benchmarks are not part of CI. `cargo bench` runs criterion benches (and iai-callgrind benches in `bitm`); the benchmark programs are run manually (see `--help`).
- Keep the code compilable and runnable on 32-bit-addressing targets (for example `wasm32-wasip1`), although 64-bit CPUs are the primary optimization target.
- For changes in performance-critical code, do not guess: compare generated assembly with `internal/asmview/asm.py` (`save`, `diff`, `git_<ref>:<filter>`) or `cargo asm`, and report what was compared.
- Prefer measuring over assuming. BSuccinct is size- and speed-oriented: do not trade memory or speed for style.

## Code and documentation

- Document every item, including non-public ones (`pub(crate)` structs and fields, private functions): what it represents, what invariants it keeps, what it returns. Do not restate the signature, do not pad with prose.
- `unsafe` functions and blocks require a `# Safety` explanation of the obligations of the caller or of the code itself.
- Short, runnable examples are welcome in `///` docs (`# Example`) and in READMEs; they are doctested.
- Formatting: rustfmt defaults (there is no `rustfmt.toml`). Format only the code you touch; never reformat unrelated code.
- The edition may be bumped when a change genuinely needs it, but not gratuitously. The MSRV is the latest stable Rust (it is not pinned in `Cargo.toml`); if the installed compiler is older than the change needs, ask the user to update the toolchain instead of working around it. Avoid nightly-only features unless they sit behind an optional feature (see `sip13`).
- Comments explain *why*; code explains *what*.
- Keep the dependency footprint small: prefer `std` or already-used crates, add optional dependencies behind features, and keep the `version` alongside each `path` dependency because crates are published. A dependency version may be bumped when a change genuinely needs it, but never without such a need.
- `Cargo.toml` metadata follows the existing pattern: `license = "MIT OR Apache-2.0"`, `repository`, `documentation`, `include = [ "**/*.rs", "Cargo.toml", "README.md" ]`, `categories`, `keywords`. Do not set `readme`, it defaults to `README.md`.

## Tests

- Unit tests are `#[cfg(test)] mod tests` inside the module they test; there is no top-level `tests/` directory.
- Tests must be deterministic and fast: no timing assertions, no benchmark-like workloads; use small, fixed inputs.
- When a documented example is the natural place to exercise new behavior, put the test there.

## Scope of changes and commits

- Make the smallest change that solves the task. Do not refactor, rename, reorder or reformat unrelated code; do not bump the versions of the published crates unless asked.
- Larger code changes are fine — the user reviews every change before committing. Keep such changes reviewable: explain the intent, the approach, and anything that may affect performance or memory usage.
- Do not create commits; leave committing to the user.
- Do not rewrite the "Bibliography" and "Reproducing experiments from the papers" sections of the benchmark READMEs — they document published results.
- The user's own commit messages are English and terse, often just two or three words (e.g. `compilation fix`, `fixed warnings`); agents may write slightly longer ones, but keep them short and to the point. Work on `main`.
- Before finishing, verify: run `cargo build`/`cargo test` (plus the relevant feature combination), check that no new warnings appear, and re-run any benchmark or helper script affected by the change. State explicitly what was run and with which features.

## AI usage

- "The Use of Artificial Intelligence" in the root `README.md` describes the current, still limited use of AI agents in this project (and is expected to evolve as their quality improves); it is not a set of rules for agents.
- Prefer documentation, tests and scripts, but do not avoid code changes when they are needed; just keep them explainable and reviewable.
