/* Mixed "realistic" workload benchmark, matching
 * benchmarks/realistic/realistic-array.lisp's run-arr-once / bench-arr:
 *
 *   1. array processing (n=300): square 1..n, sum evens, +2n
 *   2. key/value lookup: build {1:3,...,250:750}, 1500 lookups, sum
 *   3. naive recursive fibonacci(23)
 *   4. tail-recursive sum 1..300
 *   5. ackermann(2, 50)
 *   6. record processing (n=300): categorize into 3 buckets, totals+count+max
 *
 * run_once sums all six; the benchmark repeats run_once 10 times and sums
 * those into a checksum.
 *
 * Build:
 *   gcc -O2 -o realistic realistic.c
 * Run:
 *   ./realistic
 */
#include <stdint.h>
#include <stdio.h>
#include <time.h>

#define ARR_N 300
#define KV_N 250
#define KV_REPS 1500
#define REC_N 300

static long bench_array_lists(int n) {
    long arr[ARR_N];
    for (int i = 0; i < n; i++) {
        arr[i] = (long)(i + 1) * (long)(i + 1);
    }
    long total = 0;
    for (int i = 0; i < n; i++) {
        if (arr[i] % 2 == 0) {
            total += arr[i];
        }
    }
    return total + 2L * n;
}

static long bench_kv_lookup(int n, int reps) {
    long h[KV_N + 1];
    for (int i = 1; i <= n; i++) {
        h[i] = (long)i * 3;
    }
    long total = 0;
    for (int i = 0; i < reps; i++) {
        int key = 1 + (i % n);
        total += h[key];
    }
    return total;
}

static long fib(int n) {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

static long tsum(int n, long acc) {
    if (n == 0) {
        return acc;
    }
    return tsum(n - 1, acc + n);
}

static long ackermann(long m, long n) {
    if (m == 0) {
        return n + 1;
    }
    if (n == 0) {
        return ackermann(m - 1, 1);
    }
    return ackermann(m - 1, ackermann(m, n - 1));
}

static long process_records(int n) {
    long t1 = 0, t2 = 0, t3 = 0, mx = 0;
    for (int i = 0; i < n; i++) {
        long amt = ((long)(i + 1) * 7) % 100;
        int cat = 1 + ((i + 1) % 3);
        if (amt > mx) {
            mx = amt;
        }
        if (cat == 1) {
            t1 += amt;
        } else if (cat == 2) {
            t2 += amt;
        } else {
            t3 += amt;
        }
    }
    return t1 + t2 + t3 + n + mx;
}

static long run_once(void) {
    return bench_array_lists(ARR_N) + bench_kv_lookup(KV_N, KV_REPS) +
           fib(23) + tsum(300, 0) + ackermann(2, 50) + process_records(REC_N);
}

static uint64_t monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

int main(void) {
    const int reps = 10;
    long checksum = 0;

    uint64_t start = monotonic_ns();
    for (int i = 0; i < reps; i++) {
        checksum += run_once();
    }
    uint64_t elapsed_ns = monotonic_ns() - start;
    double ms = (double)elapsed_ns / 1e6;

    printf("result=%ld time=%.1f ms (%d reps)\n", checksum, ms, reps);
    return 0;
}
