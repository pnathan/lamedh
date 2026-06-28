#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static uint64_t fibonacci(uint32_t n) {
    switch (n) {
    case 0:
        return 0;
    case 1:
        return 1;
    default:
        return fibonacci(n - 1) + fibonacci(n - 2);
    }
}

static uint64_t fibonacci_sum(uint32_t n) {
    uint64_t result = 0;
    for (uint32_t i = 1; i < n; i++) {
        result += fibonacci(i);
    }
    return result;
}

static uint64_t monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

static double std_dev(const double *times, size_t len, double mean) {
    if (len < 2) {
        return 0.0;
    }
    double sum = 0.0;
    for (size_t i = 0; i < len; i++) {
        double diff = times[i] - mean;
        sum += diff * diff;
    }
    return sqrt(sum / (double)(len - 1));
}

static uint64_t bench(uint64_t run_ms, uint32_t n, int emit) {
    uint64_t run_ns = run_ms * 1000000ull;
    uint64_t elapsed_total = 0;
    uint64_t result = 0;
    size_t len = 0;
    size_t cap = 64;
    double *times = malloc(cap * sizeof(double));
    if (!times) {
        fprintf(stderr, "malloc failed\n");
        exit(1);
    }

    while (elapsed_total < run_ns) {
        uint64_t start = monotonic_ns();
        result = fibonacci_sum(n);
        uint64_t elapsed = monotonic_ns() - start;
        elapsed_total += elapsed;

        if (len == cap) {
            cap *= 2;
            double *next = realloc(times, cap * sizeof(double));
            if (!next) {
                free(times);
                fprintf(stderr, "realloc failed\n");
                exit(1);
            }
            times = next;
        }
        times[len++] = (double)elapsed / 1000000.0;
    }

    if (emit) {
        double sum = 0.0;
        double min = INFINITY;
        double max = -INFINITY;
        for (size_t i = 0; i < len; i++) {
            sum += times[i];
            if (times[i] < min) {
                min = times[i];
            }
            if (times[i] > max) {
                max = times[i];
            }
        }
        double mean = sum / (double)len;
        printf("%.6f,%.6f,%.6f,%.6f,%zu,%llu\n", mean,
               std_dev(times, len, mean), min, max, len,
               (unsigned long long)result);
    }

    free(times);
    return result;
}

int main(int argc, char **argv) {
    if (argc != 4) {
        fprintf(stderr, "Usage: %s <run_ms> <warmup_ms> <n>\n", argv[0]);
        return 1;
    }

    uint64_t run_ms = strtoull(argv[1], NULL, 10);
    uint64_t warmup_ms = strtoull(argv[2], NULL, 10);
    uint32_t n = (uint32_t)strtoul(argv[3], NULL, 10);

    if (warmup_ms > 0) {
        bench(warmup_ms, n, 0);
    }
    if (run_ms > 0) {
        bench(run_ms, n, 1);
    }
    return 0;
}
