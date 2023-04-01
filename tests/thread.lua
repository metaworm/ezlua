
-- assert(__file__():find 'thread.lua$')

local threads = {}
local tt = { n = 0 }
local count = 64
for i = 1, count do
    threads[i] = thread.spawn(function()
        tt.n = tt.n + 1
        print(tt.n)
    end)
end

for i, t in ipairs(threads) do
    t:join()
    print('#' .. i .. ' finished')
end
assert(tt.n == count)

local cond = thread.condvar()
local val = 'notify: 111'
local condthread = thread.spawn(function()
    assert(val == cond:wait())
end)

thread.sleep(100)
cond:notify_one(val)
condthread:join()