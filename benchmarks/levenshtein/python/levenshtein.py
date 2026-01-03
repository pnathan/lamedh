#!/usr/bin/env python3
"""
Levenshtein distance benchmark for Python
Matches the pattern from bddicken/languages
"""

import sys
import time


def levenshtein_distance(str1: str, str2: str) -> int:
    """Calculate Levenshtein distance between two strings"""
    if len(str2) < len(str1):
        return levenshtein_distance(str2, str1)

    m, n = len(str1), len(str2)

    prev = [0] * (m + 1)
    curr = [0] * (m + 1)

    for i in range(m + 1):
        prev[i] = i

    # Compute Levenshtein distance
    for i in range(1, n + 1):
        curr[0] = i
        for j in range(1, m + 1):
            # Cost is 0 if characters match, 1 if they differ
            cost = 0 if str1[j-1] == str2[i-1] else 1
            curr[j] = min(
                prev[j] + 1,        # Deletion
                curr[j-1] + 1,      # Insertion
                prev[j-1] + cost    # Substitution
            )
        prev, curr = curr, prev

    return prev[m]


def calculate_distances(words):
    """Calculate pairwise Levenshtein distances"""
    results = []
    for i in range(len(words) - 1):
        for j in range(i + 1, len(words)):
            distance = levenshtein_distance(words[i], words[j])
            results.append(distance)
    return results


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
        print(f"Usage: {sys.argv[0]} <run_ms> <warmup_ms> <input_file>", file=sys.stderr)
        sys.exit(1)

    run_ms = int(sys.argv[1])
    warmup_ms = int(sys.argv[2])
    input_file = sys.argv[3]

    # Read words from file
    with open(input_file, 'r') as f:
        words = [line.strip() for line in f if line.strip()]

    def benchmark_func():
        distances = calculate_distances(words)
        return sum(distances)

    # Warmup
    if warmup_ms > 0:
        bench(warmup_ms, benchmark_func)

    # Actual benchmark
    if run_ms > 0:
        data = bench(run_ms, benchmark_func)
        print(format_bench(data))


if __name__ == "__main__":
    main()
