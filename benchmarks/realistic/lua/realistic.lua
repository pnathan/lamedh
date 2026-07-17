#!/usr/bin/env lua
-- Mixed "realistic" workload benchmark, mirroring
-- benchmarks/realistic/ruby/realistic.rb (same six parts, same checksum).

local ARR_N, KV_N, KV_REPS, REC_N = 300, 250, 1500, 300

local function bench_array_lists(n)
  local arr = {}
  for i = 1, n do arr[i] = i * i end
  local total = 0
  for i = 1, n do
    if arr[i] % 2 == 0 then total = total + arr[i] end
  end
  return total + 2 * n
end

local function bench_kv_lookup(n, reps)
  local h = {}
  for i = 1, n do h[i] = i * 3 end
  local total = 0
  for i = 0, reps - 1 do
    total = total + h[1 + (i % n)]
  end
  return total
end

local function fib(n)
  if n < 2 then return n end
  return fib(n - 1) + fib(n - 2)
end

local function tsum(n, acc)
  if n == 0 then return acc end
  return tsum(n - 1, acc + n)
end

local function ackermann(m, n)
  if m == 0 then return n + 1 end
  if n == 0 then return ackermann(m - 1, 1) end
  return ackermann(m - 1, ackermann(m, n - 1))
end

local function process_records(n)
  local t1, t2, t3, mx = 0, 0, 0, 0
  for i = 0, n - 1 do
    local amt = ((i + 1) * 7) % 100
    local cat = 1 + ((i + 1) % 3)
    if amt > mx then mx = amt end
    if cat == 1 then t1 = t1 + amt
    elseif cat == 2 then t2 = t2 + amt
    else t3 = t3 + amt end
  end
  return t1 + t2 + t3 + n + mx
end

local function run_once()
  return bench_array_lists(ARR_N)
    + bench_kv_lookup(KV_N, KV_REPS)
    + fib(23)
    + tsum(300, 0)
    + ackermann(2, 50)
    + process_records(REC_N)
end

local reps = 10
local checksum = 0
local start = os.clock()
for _ = 1, reps do checksum = checksum + run_once() end
local ms = (os.clock() - start) * 1000.0

print(string.format("checksum=%d time=%.1f ms (%d reps)", checksum, ms, reps))
