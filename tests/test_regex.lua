local regex = require 'regex'
local reg = regex.new[[(\w+)\s+(\w+)]]
print('reg', type(reg), getmetatable(reg))
do
    local cap<close> = reg:capture 'abc def'
    assert(#cap == 3)
    print(cap(0), cap(1))
    print('cap1', cap(1):range())
    assert(cap[1] == 'abc')
    assert(cap[2] == 'def')
    assert(cap(2).start == 4)
    local cap0, cap1 = reg:match 'abc def'
    assert(cap0 == 'abc')
    assert(cap1 == 'def')
end

print '---------- replace ----------'
local replaced = reg:replace('abc def', function(c)
    assert(#c == 3, c[1] == 'abc')
    return '111'
end)
print(replaced)
assert(replaced == '111')

print '---------- gsplit ----------'
reg = regex.new[[\s+]]
for m in reg:gsplit('abc def ghi') do
    print(m)
end