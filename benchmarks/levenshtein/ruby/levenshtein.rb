#!/usr/bin/env ruby
# frozen_string_literal: true
#
# Levenshtein distance benchmark: two-row DP, "kitten" -> "sitting".

def levenshtein(s1, s2)
  s1, s2 = s2, s1 if s2.length < s1.length

  len1 = s1.length
  len2 = s2.length
  prev = (0..len1).to_a
  curr = Array.new(len1 + 1, 0)

  (0...len2).each do |i|
    curr[0] = i + 1
    (0...len1).each do |j|
      cost = s1[j] == s2[i] ? 0 : 1
      del = prev[j + 1] + 1
      ins = curr[j] + 1
      sub = prev[j] + cost
      curr[j + 1] = [del, ins, sub].min
    end
    prev, curr = curr, prev
  end

  prev[len1]
end

s1 = 'kitten'
s2 = 'sitting'
iters = 10_000
result = 0

start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
iters.times { result = levenshtein(s1, s2) }
elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - start
ms = elapsed * 1000.0

puts format('result=%d time=%.1f ms (%d iters)', result, ms, iters)
