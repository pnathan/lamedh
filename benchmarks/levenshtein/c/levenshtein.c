/* Levenshtein distance benchmark: two-row DP, "kitten" -> "sitting".
 *
 * Build:
 *   gcc -O2 -o levenshtein levenshtein.c
 * Run:
 *   ./levenshtein
 */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#define MAX_LEN 64

/* Not inlined so the compiler can't fold the whole benchmark loop into a
 * single compile-time constant across calls. */
__attribute__((noinline)) static int levenshtein(const char *s1, size_t len1,
                                                   const char *s2,
                                                   size_t len2) {
    int prev[MAX_LEN + 1];
    int curr[MAX_LEN + 1];

    for (size_t j = 0; j <= len2; j++) {
        prev[j] = (int)j;
    }

    for (size_t i = 1; i <= len1; i++) {
        curr[0] = (int)i;
        for (size_t j = 1; j <= len2; j++) {
            int cost = (s1[i - 1] == s2[j - 1]) ? 0 : 1;
            int del = prev[j] + 1;
            int ins = curr[j - 1] + 1;
            int sub = prev[j - 1] + cost;
            int m = del < ins ? del : ins;
            curr[j] = m < sub ? m : sub;
        }
        memcpy(prev, curr, (len2 + 1) * sizeof(int));
    }

    return prev[len2];
}

static uint64_t monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

int main(void) {
    /* Read the words through a volatile pointer so the compiler can't prove
     * their contents at compile time and constant-fold the whole loop. */
    static const volatile char *vs1 = "kitten";
    static const volatile char *vs2 = "sitting";
    char s1[MAX_LEN], s2[MAX_LEN];
    size_t len1 = 0, len2 = 0;

    while (vs1[len1] != '\0') {
        s1[len1] = vs1[len1];
        len1++;
    }
    while (vs2[len2] != '\0') {
        s2[len2] = vs2[len2];
        len2++;
    }

    const int iters = 10000;
    int result = 0;

    uint64_t start = monotonic_ns();
    for (int i = 0; i < iters; i++) {
        result = levenshtein(s1, len1, s2, len2);
    }
    uint64_t elapsed_ns = monotonic_ns() - start;
    double ms = (double)elapsed_ns / 1e6;

    printf("result=%d time=%.1f ms (%d iters)\n", result, ms, iters);
    return 0;
}
