# Tri-implementation benchmark summary

## Startup (empty program)

| host | median | min | stddev |
|---|---:|---:|---:|
| rust | 281.1 ms | 234.0 ms | 51.5 ms |
| sbcl | 1085.0 ms | 995.4 ms | 70.1 ms |
| asm | 6.2 ms | 5.8 ms | 1.4 ms |

## Kernel times (median of runs, startup subtracted)

Cell: net median seconds (raw median, min) and speed relative to rust-interp. `~` = within 3σ of startup noise (startup-bound). `—` = not expressible on that tier.

| kernel | rust-interp | rust-default | rust-jit | sbcl | asm |
|---|---:|---:|---:|---:|---:|
| ack | 4.134 s (4.415, 4.140) 1.0× | ~-0.005 s (0.276, 0.253) *startup-bound* | ~0.012 s (0.294, 0.259) *startup-bound* | 2.535 s (3.620, 3.396) **1.6×** | 0.037 s (0.044, 0.040) **110.6×** |
| closures | 7.819 s (8.100, 7.769) 1.0× | 7.804 s (8.085, 7.797) **1.0×** | — | 3.492 s (4.577, 4.515) **2.2×** | 0.222 s (0.228, 0.184) **35.3×** |
| fib | 6.090 s (6.371, 6.274) 1.0× | 6.085 s (6.366, 6.005) **1.0×** | ~0.051 s (0.332, 0.295) *startup-bound* | 4.700 s (5.785, 5.307) **1.3×** | 0.077 s (0.084, 0.067) **78.7×** |
| lists | 8.478 s (8.759, 8.493) 1.0× | 8.781 s (9.062, 8.440) **1.0×** | — | 5.412 s (6.497, 6.242) **1.6×** | 0.294 s (0.301, 0.280) **28.8×** |
| mandel | 11.539 s (11.820, 11.087) 1.0× | 0.417 s (0.698, 0.609) **27.7×** | ~-0.004 s (0.277, 0.271) *startup-bound* | 6.410 s (7.495, 7.449) **1.8×** | 0.944 s (0.951, 0.914) **12.2×** |
| msort | 11.376 s (11.657, 11.452) 1.0× | 11.567 s (11.848, 11.140) **1.0×** | — | 5.567 s (6.652, 6.506) **2.0×** | 0.533 s (0.539, 0.433) **21.3×** |
| prefix | 9.975 s (10.256, 9.756) 1.0× | ~0.025 s (0.306, 0.251) *startup-bound* | ~-0.013 s (0.269, 0.256) *startup-bound* | 5.931 s (7.016, 6.622) **1.7×** | 0.112 s (0.118, 0.116) **89.4×** |
| sieve | 12.573 s (12.854, 12.616) 1.0× | ~0.030 s (0.311, 0.300) *startup-bound* | ~0.029 s (0.310, 0.282) *startup-bound* | 7.591 s (8.676, 8.467) **1.7×** | 0.159 s (0.165, 0.153) **78.9×** |
| strings | 0.425 s (0.706, 0.687) 1.0× | 0.407 s (0.688, 0.675) **1.0×** | — | 0.625 s (1.710, 1.615) **0.7×** | 2.183 s (2.190, 2.148) **0.2×** |
| tailsum | 10.009 s (10.290, 9.802) 1.0× | ~0.026 s (0.307, 0.261) *startup-bound* | ~0.001 s (0.282, 0.253) *startup-bound* | 8.208 s (9.293, 8.881) **1.2×** | 0.075 s (0.081, 0.077) **133.9×** |
| whileloop | 6.998 s (7.279, 7.109) 1.0× | ~-0.018 s (0.263, 0.250) *startup-bound* | ~-0.024 s (0.257, 0.251) *startup-bound* | 4.211 s (5.296, 4.962) **1.7×** | 0.058 s (0.064, 0.058) **120.1×** |

## Native tiers at scaled inputs (asm vs typed JIT)

Same kernels with the `;; scaled:` input (see each kernel file). Ratio = net(asm) / net(rust-jit): >1 means the typed JIT is faster.

| kernel | rust-jit-scaled | asm-scaled | JIT speedup over asm |
|---|---:|---:|---:|
| ack | 1.766 s (2.047, 1.946) | 2.370 s (2.376, 2.337) | **1.3×** |
| fib | 1.256 s (1.537, 1.483) | 1.102 s (1.108, 1.064) | **0.9×** |
| mandel | ~0.146 s (0.428, 0.402) | 6.564 s (6.571, 6.539) | **44.8×** |
| prefix | 0.187 s (0.468, 0.446) | 1.001 s (1.007, 0.929) | **5.3×** |
| sieve | 0.523 s (0.804, 0.791) | 1.863 s (1.869, 1.662) | **3.6×** |
| tailsum | 0.304 s (0.585, 0.522) | 2.575 s (2.582, 2.342) | **8.5×** |
| whileloop | 0.458 s (0.739, 0.714) | 2.153 s (2.159, 2.004) | **4.7×** |

## Output agreement

All implementations printed identical results for every kernel.
