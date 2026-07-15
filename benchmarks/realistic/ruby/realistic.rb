#!/usr/bin/env ruby
# frozen_string_literal: true
#
# Mixed "realistic" workload benchmark, matching
# benchmarks/realistic/realistic-array.lisp's run-arr-once / bench-arr:
#
#   1. array processing (n=300): square 1..n, sum evens, +2n
#   2. key/value lookup: build {1:3,...,250:750}, 1500 lookups, sum
#   3. naive recursive fibonacci(23)
#   4. tail-recursive sum 1..300
#   5. ackermann(2, 50)
#   6. record processing (n=300): categorize into 3 buckets, totals+count+max
#
# run_once sums all six; the benchmark repeats run_once 10 times and sums
# those into a checksum.

ARR_N = 300
KV_N = 250
KV_REPS = 1500
REC_N = 300

def bench_array_lists(n)
  arr = (1..n).map { |i| i * i }
  total = arr.select(&:even?).sum
  total + (2 * n)
end

def bench_kv_lookup(n, reps)
  h = {}
  (1..n).each { |i| h[i] = i * 3 }
  total = 0
  (0...reps).each do |i|
    key = 1 + (i % n)
    total += h[key]
  end
  total
end

def fib(n)
  n < 2 ? n : fib(n - 1) + fib(n - 2)
end

def tsum(n, acc)
  n.zero? ? acc : tsum(n - 1, acc + n)
end

def ackermann(m, n)
  if m.zero?
    n + 1
  elsif n.zero?
    ackermann(m - 1, 1)
  else
    ackermann(m - 1, ackermann(m, n - 1))
  end
end

def process_records(n)
  t1 = 0
  t2 = 0
  t3 = 0
  mx = 0
  (0...n).each do |i|
    amt = ((i + 1) * 7) % 100
    cat = 1 + ((i + 1) % 3)
    mx = amt if amt > mx
    case cat
    when 1 then t1 += amt
    when 2 then t2 += amt
    else t3 += amt
    end
  end
  t1 + t2 + t3 + n + mx
end

def run_once
  bench_array_lists(ARR_N) +
    bench_kv_lookup(KV_N, KV_REPS) +
    fib(23) +
    tsum(300, 0) +
    ackermann(2, 50) +
    process_records(REC_N)
end

reps = 10
checksum = 0

start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
reps.times { checksum += run_once }
elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - start
ms = elapsed * 1000.0

puts format('result=%d time=%.1f ms (%d reps)', checksum, ms, reps)
