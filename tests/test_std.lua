print(os.name, __file__())

local cmd
if os.name == 'windows' then
    function cmd(...) return os.command {'cmd', '/c', ...} end
else
    function cmd(...) return os.command {...} end
end

local out, err = cmd('echo', '1234'):arg'5678':stdout'pipe':spawn():wait_output()
out = out:trim()
print(out, err)

assert(out == '1234 5678')
print(readfile('/path/not/exists'))

for entry in os.read_dir('.') do
    print(entry.file_name, entry.path, entry.metadata)
end