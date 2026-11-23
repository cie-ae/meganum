"""Square four 200,000-digit MegaInt values concurrently and sequentially.

Spawns four threading.Thread workers, each squaring a distinct operand.
The GIL is released inside the native engine for heavy operations, so the
native work overlaps. Wall-clock times for the concurrent run and for a
sequential loop are measured with time.perf_counter, and the speedup is
printed with an honest note: scaling depends on the number of physical
cores and on machine load, so the observed ratio varies between machines.

Usage:
    python threading.py
"""

import sys
import threading
import time

from meganum import MegaInt

N_THREADS = 4
DIGITS = 200_000


def square_worker(index, operands, results):
    """Square operands[index] and store the result at results[index]."""
    results[index] = operands[index] * operands[index]


def run_parallel(operands):
    """Square every operand on its own thread; return results and wall time."""
    results = [None] * len(operands)
    threads = [
        threading.Thread(target=square_worker, args=(i, operands, results))
        for i in range(len(operands))
    ]
    start = time.perf_counter()
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    return results, time.perf_counter() - start


def run_sequential(operands):
    """Square every operand in one loop; return results and wall time."""
    start = time.perf_counter()
    results = [x * x for x in operands]
    return results, time.perf_counter() - start


def main():
    base = MegaInt("7" * DIGITS)
    operands = [base + MegaInt(i) for i in range(N_THREADS)]

    # Warm up the native engine before measuring either path.
    run_sequential(operands[:1])

    par_results, par_time = run_parallel(operands)
    seq_results, seq_time = run_sequential(operands)

    identical = all(p == s for p, s in zip(par_results, seq_results))
    print(f"squared {N_THREADS} distinct {DIGITS}-digit values")
    print(f"parallel wall time:   {par_time * 1000:.1f} ms")
    print(f"sequential wall time: {seq_time * 1000:.1f} ms")
    if par_time > 0:
        print(f"speedup: {seq_time / par_time:.2f}x")
    print("parallel results identical to sequential:", identical)
    print("note: scaling depends on core count and load; the GIL is "
          "released inside the native engine for heavy operations")
    return 0


if __name__ == "__main__":
    sys.exit(main())
