#!/usr/bin/env ruby
# frozen_string_literal: true

def fibonacci(n)
  return 0 if n == 0
  return 1 if n == 1

  fibonacci(n - 1) + fibonacci(n - 2)
end

def fibonacci_sum(n)
  result = 0
  (1...n).each { |i| result += fibonacci(i) }
  result
end

def std_dev(times)
  return 0.0 if times.length < 2

  mean = times.sum / times.length.to_f
  variance = times.sum { |t| (t - mean) * (t - mean) } / (times.length - 1).to_f
  Math.sqrt(variance)
end

def bench(run_ms, n)
  run_ns = run_ms * 1_000_000
  elapsed_total = 0
  times = []
  result = nil

  while elapsed_total < run_ns
    start = Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond)
    result = fibonacci_sum(n)
    elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond) - start
    elapsed_total += elapsed
    times << elapsed / 1_000_000.0
  end

  [times, result]
end

if ARGV.length != 3
  warn "Usage: #{$PROGRAM_NAME} <run_ms> <warmup_ms> <n>"
  exit 1
end

run_ms = ARGV[0].to_i
warmup_ms = ARGV[1].to_i
n = ARGV[2].to_i

bench(warmup_ms, n) if warmup_ms.positive?

if run_ms.positive?
  times, result = bench(run_ms, n)
  mean = times.sum / times.length.to_f
  puts format('%.6f,%.6f,%.6f,%.6f,%d,%d',
              mean, std_dev(times), times.min, times.max, times.length, result)
end
