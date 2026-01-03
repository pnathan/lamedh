#!/usr/bin/env python3
"""Benchmark harness for lamedh Fibonacci implementation"""

import sys
import time
import subprocess
import os

def variance(array):
    if len(array) < 2:
        return 0.0
    mean = sum(array) / len(array)
    return sum((x - mean) ** 2 for x in array) / (len(array) - 1)

def std_dev(array):
    return variance(array) ** 0.5

def run_lisp_benchmark(n):
    """Run the fibonacci benchmark in lamedh and return the result"""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    lamedh_path = os.path.join(script_dir, "../../../target/release/lamedh")
    lisp_file = os.path.join(script_dir, "fibonacci.lisp")

    cmd = [lamedh_path, "-i", lisp_file, "-s", f"(fibonacci-sum {n})"]

    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"Lisp execution failed: {result.stderr}")

    # Parse the output - last line should be the result
    output = result.stdout.strip()
    return int(output) if output else 0

def bench(run_ms, n):
    times = []
    result = None
    run_ns = run_ms * 1_000_000

    while sum(times) < run_ns:
        start = time.monotonic_ns()
        result = run_lisp_benchmark(n)
        end = time.monotonic_ns()
        elapsed = end - start
        times.append(elapsed)

        if run_ms > 1 and (sum(times) // 1_000_000_000) > (sum(times[:-1]) // 1_000_000_000):
            sys.stderr.write('.')
            sys.stderr.flush()

    if run_ms > 1:
        sys.stderr.write('\n')

    return {
        'times': times,
        'result': result
    }

def format_bench(data):
    if not data['times']:
        raise ValueError("no data!")

    result = data['result']
    times = [t / 1_000_000 for t in data['times']]  # convert to milliseconds

    # mean_ms,std-dev-ms,min_ms,max_ms,times,result
    return f"{sum(times) / len(times)},{std_dev(times)},{min(times)},{max(times)},{len(times)},{result}"

def main():
    if len(sys.argv) != 4:
        print(f"Usage: {sys.argv[0]} <run_ms> <warmup_ms> <n>", file=sys.stderr)
        sys.exit(1)

    run_ms = int(sys.argv[1])
    warmup_ms = int(sys.argv[2])
    n = int(sys.argv[3])

    # Build lamedh if needed
    script_dir = os.path.dirname(os.path.abspath(__file__))
    lamedh_path = os.path.join(script_dir, "../../../target/release/lamedh")

    if not os.path.exists(lamedh_path):
        print("Building lamedh...", file=sys.stderr)
        subprocess.run(["cargo", "build", "--release"],
                      cwd=os.path.join(script_dir, "../../.."),
                      check=True)

    # Warmup
    if warmup_ms > 0:
        bench(warmup_ms, n)

    # Actual benchmark
    if run_ms > 0:
        data = bench(run_ms, n)
        print(format_bench(data))

if __name__ == "__main__":
    main()
