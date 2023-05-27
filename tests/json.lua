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