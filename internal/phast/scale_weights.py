#!/usr/bin/env python3
"""Rescale the PHast bucket-evaluator weight tables in `ph/src/phast`.

Every array literal that holds exactly 7 integer numbers is treated as a bucket-weight
table (see `Weights` in `ph/src/phast/evaluator.rs`) and each of its numbers is multiplied
by `--factor` (default: 4). Nothing else in a file is touched: the separators (commas,
spaces, line breaks) are preserved, so the diff contains only the changed numbers.

Why: `Weights::eval` subtracts `1024 * bucket_nr`. Multiplying both the weights and that
constant by the same factor keeps the evaluation order unchanged (integer arithmetic),
while giving the weights more resolution.

The script is NOT idempotent: running it twice multiplies by `factor**2`.
Revert the tables with `git checkout -- ph/src/phast`.

Usage (from anywhere in the repository):
    ./internal/phast/scale_weights.py --dry-run     # report only, do not write
    ./internal/phast/scale_weights.py               # multiply every weight by 4
    ./internal/phast/scale_weights.py -f 2 ph/src/phast/seed_chooser/k.rs
"""

import argparse
import re
import sys
from pathlib import Path

# Range of the weights, which are stored as `i32` (see `Weights` in `ph/src/phast/evaluator.rs`).
I32_MIN = -(2**31)
I32_MAX = 2**31 - 1

# Array literal of exactly 7 integers, i.e. the weights of buckets of size 1..7.
# Whitespace (including line breaks) is allowed around the numbers and the commas.
ARRAY_RE = re.compile(r'\[\s*-?\d[\d_]*(?:\s*,\s*-?\d[\d_]*){6}\s*\]')

# Single integer, possibly negative and possibly with `_` digit separators.
INT_RE = re.compile(r'-?\d[\d_]*')


def default_dir() -> Path:
    """Returns the directory with the weight tables (`<repo>/ph/src/phast`),
    inferred from the location of this script (`<repo>/internal/phast/scale_weights.py`)."""
    return Path(__file__).resolve().parents[2] / 'ph' / 'src' / 'phast'


def rs_files(paths):
    """Yields `*.rs` files from the given `paths`:
    directories are searched recursively, files are yielded as is."""
    for path in paths:
        if path.is_dir():
            yield from sorted(path.rglob('*.rs'))
        else:
            yield path


def scale(text: str, factor: int) -> str:
    """Returns `text` with the numbers of all 7-element array literals multiplied by `factor`."""
    def scale_array(match) -> str:
        return INT_RE.sub(lambda number: str(int(number.group(0).replace('_', '')) * factor), match.group(0))
    return ARRAY_RE.sub(scale_array, text)


def weights(text: str):
    """Yields the values of all numbers of all 7-element array literals in `text`."""
    for match in ARRAY_RE.finditer(text):
        for number in INT_RE.findall(match.group(0)):
            yield int(number.replace('_', ''))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('paths', nargs='*', type=Path,
                        help='files or directories to process (default: <repo>/ph/src/phast)')
    parser.add_argument('-f', '--factor', type=int, default=4, help='integer multiplier (default: 4)')
    parser.add_argument('-n', '--dry-run', action='store_true', help='report the changes without writing the files')
    args = parser.parse_args()

    if args.factor == 0:
        parser.error('--factor must be non-zero')

    files = 0
    arrays = 0
    failed = False
    for path in rs_files(args.paths or [default_dir()]):
        text = path.read_text(encoding='utf-8')
        num_of_arrays = len(ARRAY_RE.findall(text))
        if num_of_arrays == 0:
            continue
        scaled = scale(text, args.factor)
        # The weights are `i32`, so the scaling must not overflow them.
        out_of_range = [v for v in weights(scaled) if not I32_MIN <= v <= I32_MAX]
        if out_of_range:
            print(f'{path}: {len(out_of_range)} weight(s) do not fit in i32 after scaling '
                  f'(first: {out_of_range[0]}), file skipped', file=sys.stderr)
            failed = True
            continue
        action = 'would rescale' if args.dry_run else 'rescaled'
        print(f'{path}: {action} {num_of_arrays} array(s)')
        files += 1
        arrays += num_of_arrays
        if not args.dry_run and scaled != text:
            # `newline=''` keeps the original line endings.
            with open(path, 'w', encoding='utf-8', newline='') as output:
                output.write(scaled)

    action = 'to rescale' if args.dry_run else 'rescaled'
    print(f'{files} file(s), {arrays} array(s) {action} by {args.factor}')
    if arrays != 0 and not args.dry_run:
        print('Now update the `1024` constant in `ph/src/phast/evaluator.rs` (`Weights::eval`)')
        print('and the direct-weight bounds in `internal/phast/src/optim.rs` accordingly.')
        print('Revert the tables with: git checkout -- ph/src/phast')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
