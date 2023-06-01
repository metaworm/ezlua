local t = os.clock()
for i = 1,10000 do
    add(i, i + 1)
end
print('number function consumed', os.clock() - t)

local t = os.clock()
for i = 1,10000 do
    strsub('1234567890qwertyuiopasdfghjklzxcvbnm', i % 30)
end
print('string function consumed', os.clock() - t)

local t = os.clock()
for i = 1,10000 do
    getTest()
end
print('create userdata consumed', os.clock() - t)

local t = os.clock()
local u = getTest()
for i = 1,10000 do
    u.a = 1000
end
print('userdata setter consumed', os.clock() - t)

local t = os.clock()
for i = 1,10000 do
    assert(u.a == 1000)
end
print('userdata getter consumed', os.clock() - t)

local t = os.clock()
for i = 1,10000 do
    u:inc()
end
print('userdata method consumed', os.clock() - t)