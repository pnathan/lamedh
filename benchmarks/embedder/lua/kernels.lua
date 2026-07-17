#!/usr/bin/env lua
-- The embedder bench's B/C scenarios in Lua, so the "Call Me Maybe" table
-- can compare compiled Lamedh against the embeddable-language standard:
-- run under BOTH `lua5.4` (baseline interpreter) and `luajit` (tracing JIT).
--
--   B. sphere-SDF kernel, 1,000,000 evaluations (matches KERNEL-LOOP-T)
--   C. dot product over 100,000 doubles      (matches DOT-T)

local sqrt = math.sqrt

local function sd_sphere(x, y, z)
  return sqrt(x * x + y * y + z * z) - 1.0
end

local function kernel_loop(n)
  local acc = 0.0
  for i = 0, n - 1 do
    acc = acc + sd_sphere(0.001 * i, 0.5, 0.25)
  end
  return acc
end

-- warmup (lets LuaJIT trace-compile before measurement)
kernel_loop(100000)

local n = 1000000
local start = os.clock()
local acc = kernel_loop(n)
local ns_per = (os.clock() - start) * 1e9 / n
print(string.format("B  sdf-kernel   %8.1f ns/eval   (sum %.1f)", ns_per, acc))

local m = 100000
local a, b = {}, {}
for i = 1, m do
  a[i] = 0.5 * (i - 1)
  b[i] = 2.0
end

local function dot(a, b, m)
  local acc = 0.0
  for i = 1, m do acc = acc + a[i] * b[i] end
  return acc
end

dot(a, b, m) -- warmup

start = os.clock()
local iters = 100
local d = 0.0
for _ = 1, iters do d = dot(a, b, m) end
local ns_elem = (os.clock() - start) * 1e9 / (m * iters)
print(string.format("C  dot-product  %8.1f ns/elem   (%.1f)", ns_elem, d))
