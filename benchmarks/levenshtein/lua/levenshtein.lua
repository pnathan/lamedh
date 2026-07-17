#!/usr/bin/env lua
-- Levenshtein distance benchmark: two-row DP, "kitten" -> "sitting".
-- Mirrors benchmarks/levenshtein/ruby/levenshtein.rb exactly.

local byte = string.byte

local function levenshtein(s1, s2)
  if #s2 < #s1 then s1, s2 = s2, s1 end
  local len1, len2 = #s1, #s2
  local prev, curr = {}, {}
  for j = 0, len1 do prev[j] = j end
  for i = 0, len2 - 1 do
    curr[0] = i + 1
    local ci = byte(s2, i + 1)
    for j = 0, len1 - 1 do
      local cost = (byte(s1, j + 1) == ci) and 0 or 1
      local del = prev[j + 1] + 1
      local ins = curr[j] + 1
      local sub = prev[j] + cost
      local m = del
      if ins < m then m = ins end
      if sub < m then m = sub end
      curr[j + 1] = m
    end
    prev, curr = curr, prev
  end
  return prev[len1]
end

local s1, s2 = "kitten", "sitting"
local iters = 10000
local result = 0

local start = os.clock()
for _ = 1, iters do result = levenshtein(s1, s2) end
local ms = (os.clock() - start) * 1000.0

print(string.format("result=%d time=%.1f ms (%d iters)", result, ms, iters))
