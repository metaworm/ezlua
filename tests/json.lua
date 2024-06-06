local json = require 'json'

local val = json.load([==[
    {
        "Architecture": "x86_64",
        "BridgeNfIp6tables": true,
        "BridgeNfIptables": true,
        "CPUSet": true,
        "CPUShares": true,
        "CgroupDriver": "systemd",
        "CgroupVersion": "2"
    }
]==])
print(json.dump(val, true))

for _, data in ipairs {
    {1, nil, 3},
    {nil, nil, 3},
    -- {1, 2, nil},
    {1, 2, 3},
} do
    local text = json.dump(data)
    print(text)
    assert(json.dump(json.load(text)) == text)
end