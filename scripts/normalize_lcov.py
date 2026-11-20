#!/usr/bin/env python3
"""Normalize an lcov tracefile for the MegaNum coverage gate.

Two different tools feed the coverage gate:

* `cargo llvm-cov --lcov` (meganum-core, cargo-managed test binaries);
* `llvm-cov export --format=lcov` (meganum-python, exercised through the
  pytest suite running against an instrumented extension module).

They disagree on the `LF:`/`LH:` record summaries (observed: `llvm-cov
export` can emit `LF:` values that do not match its own `DA:` entries).
The gate aggregates `LF`/`LH` per bucket, so every record is rewritten
here to derive `LF`/`LH` from its `DA:` entries directly (duplicate `DA:`
entries for the same line are summed, per the lcov tracefile spec).

Optionally the output is filtered to records whose `SF:` path contains a
needle (e.g. `crates/meganum-python`), so the two pipelines can be
concatenated into one merged tracefile without double counting.

Usage:
    normalize_lcov.py [--needle NEEDLE] INPUT OUTPUT
"""

from __future__ import annotations

import argparse
import sys


def normalize(input_path: str, output_path: str, needle: str | None) -> tuple[int, int]:
    kept = dropped = 0
    sf: str | None = None
    das: dict[int, int] = {}
    with open(input_path, encoding="utf-8", errors="replace") as fh, open(
        output_path, "w", encoding="utf-8"
    ) as out:
        def flush() -> None:
            nonlocal kept, dropped
            if sf is None:
                return
            match = needle is None or needle in sf.replace("\\", "/")
            if match:
                kept += 1
                out.write(f"SF:{sf}\n")
                for n in sorted(das):
                    out.write(f"DA:{n},{das[n]}\n")
                out.write(f"LF:{len(das)}\n")
                out.write(f"LH:{sum(1 for h in das.values() if h > 0)}\n")
                out.write("end_of_record\n")
            else:
                dropped += 1

        for raw in fh:
            line = raw.strip()
            if line.startswith("SF:"):
                flush()
                sf, das = line[3:], {}
            elif line == "end_of_record":
                flush()
                sf, das = None, {}
            elif sf is not None and line.startswith("DA:"):
                parts = line[3:].split(",")
                if len(parts) >= 2 and parts[0].isdigit() and parts[1].lstrip("-").isdigit():
                    das[int(parts[0])] = das.get(int(parts[0]), 0) + int(parts[1])
        flush()
    return kept, dropped


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("input")
    ap.add_argument("output")
    ap.add_argument("--needle", default=None, help="keep only SF paths containing this substring")
    args = ap.parse_args()
    kept, dropped = normalize(args.input, args.output, args.needle)
    print(f"normalize_lcov: kept {kept} records, dropped {dropped}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
