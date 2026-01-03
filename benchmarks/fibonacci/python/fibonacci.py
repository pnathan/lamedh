#!/usr/bin/env python3
"""
Fibonacci benchmark for Python
Matches the pattern from bddicken/languages
"""

import sys
import time


def fibonacci(n):
    """Calculate the nth Fibonacci number using naive recursion"""
    if n == 0:
        return 0
    if n == 1:
        return 1
    return fibonacci(n-1) + fibonacci(n-2)


def variance(array):
    if len(array) < 2:
        return 0.0
    mean = sum(array) / len(array)
    return sum((x - mean) ** 2 for x in array) / (len(array) - 1)


def std_dev(array):
    return variance(array) ** 0.5


def bench(run_ms, func):
    times = []
    result = None
    run_ns = run_ms * 1_000_000

    while sum(times) < run_ns:
        start = time.monotonic_ns()
        result = func()
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

    def benchmark_func():
        result = 0
        for i in range(1, n):
            result += fibonacci(i)
        return result

    # Warmup
    if warmup_ms > 0:
        bench(warmup_ms, benchmark_func)

    # Actual benchmark
    if run_ms > 0:
        data = bench(run_ms, benchmark_func)
        print(format_bench(data))


if __name__ == "__main__":
    main()
