#!/usr/bin/env lua
-- Naive recursive fibonacci(30), warm repeated runs: 100 ms warmup, then
-- as many runs as fit in ~1000 ms; reports mean ms per run. Mirrors the
-- protocol of benchmarks/fibonacci's other language entries.

local function fib(n)
  if n < 2 then return n end
  return fib(n - 1) + fib(n - 2)
end

local N = 30
local WARMUP_S = 0.1
local RUN_S = 1.0

local t0 = os.clock()
while os.clock() - t0 < WARMUP_S do fib(N) end

local runs, result = 0, 0
local start = os.clock()
repeat
  result = fib(N)
  runs = runs + 1
until os.clock() - start >= RUN_S
local mean_ms = (os.clock() - start) * 1000.0 / runs

print(string.format("fib(%d)=%d mean=%.1f ms over %d runs", N, result, mean_ms, runs))
