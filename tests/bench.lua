local t = os.clock()
for i = 1,10000 do
    add(i, i + 1)
end
print('add consumed', os.clock() - t)

local t = os.clock()
for i = 1,10000 do
    strsub('1234567890qwertyuiopasdfghjklzxcvbnm', i % 30)
end
print('strsub consumed', os.clock() - t)