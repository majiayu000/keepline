"""Color gate for the private Claude exec wrapper. No provider secret, no model request.

offline: build the wrapper from fixture.py's template with a throwaway provider row
(unreachable base URL), run the real Claude CLI in a PTY like keeplined does, and
require colored SGR on its first screen. A run without TERM must stay monochrome,
so the gate proves it can tell the two apart.
--live ROOT: the running acceptance instance's Claude processes must carry TERM/COLORTERM.
"""
import ast, json, os, pathlib, pty, re, select, signal, sqlite3, subprocess, sys, tempfile, time
OUT = pathlib.Path(__file__).resolve().parent
CLI = str(pathlib.Path.home()/'.local/bin/claude')
PROVIDER = 'fe4d110b-81b5-43d1-80db-b42ccaeafde5'
SGR = re.compile(rb'\x1b\[([0-9;:]*)m')

def template():
    tree = ast.parse((OUT/'fixture.py').read_text())
    node = next(n for n in ast.walk(tree) if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'script' for t in n.targets))
    text = node.value.func.value.func.value.value
    assert isinstance(text, str) and 'settings_config' in text
    return text

def colored(data):
    for m in SGR.finditer(data):
        params = re.split(rb'[;:]', m.group(1))
        for i, p in enumerate(params):
            n = int(p) if p.isdigit() else -1
            if 30 <= n <= 37 or 90 <= n <= 97 or 40 <= n <= 47 or 100 <= n <= 107:
                return True
            if n in (38, 48) and i + 1 < len(params):
                return True
    return False

def first_screen(wrapper, env, seconds=12):
    pid, fd = pty.fork()
    if pid == 0:
        os.chdir(wrapper.parents[1])
        os.execve(str(wrapper), [str(wrapper)], env)
    data, end = b'', time.monotonic() + seconds
    try:
        while time.monotonic() < end and not colored(data):
            if select.select([fd], [], [], 0.2)[0]:
                try:
                    chunk = os.read(fd, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                data += chunk
                if b'\x1b[6n' in chunk:  # cursor position query; a real terminal answers
                    os.write(fd, b'\x1b[1;1R')
    finally:
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
        os.close(fd)
    return data

def offline():
    with tempfile.TemporaryDirectory(prefix='kl-color-') as d:
        root = pathlib.Path(d); (root/'bin').mkdir(); home = root/'home'; (home/'.claude').mkdir(parents=True)
        (root/'mcp.json').write_text('{"mcpServers":{}}')
        (home/'.claude/settings.json').write_text(json.dumps({'disableAllHooks': True}))
        # Past onboarding, like the screenshot. The theme picker previews colors even without TERM.
        (home/'.claude/.claude.json').write_text(json.dumps({'hasCompletedOnboarding': True, 'theme': 'dark'}))
        db = root/'providers.db'
        with sqlite3.connect(db) as c:
            c.execute('create table providers(id text, app_type text, settings_config text)')
            c.execute('insert into providers values (?,?,?)', (PROVIDER, 'claude', json.dumps({'env': {'ANTHROPIC_BASE_URL': 'http://127.0.0.1:9', 'ANTHROPIC_AUTH_TOKEN': 'color-check'}})))
        wrapper = root/'bin/claude'
        wrapper.write_text(template().replace('DB', repr(str(db))).replace('CLI', repr(CLI))); wrapper.chmod(0o700)
        # Same variables the keeplined PTY hands to the shell (pty.rs).
        shell = {'PATH': '/usr/bin:/bin', 'LANG': 'en_US.UTF-8', 'HOME': str(home), 'TERM': 'xterm-256color', 'COLORTERM': 'truecolor'}
        with_term = first_screen(wrapper, shell)
        without = first_screen(wrapper, {k: v for k, v in shell.items() if k not in ('TERM', 'COLORTERM')})
    result = {'bytes_with_term': len(with_term), 'colored_with_term': colored(with_term),
              'bytes_without_term': len(without), 'colored_without_term': colored(without)}
    print(json.dumps(result))
    assert result['bytes_with_term'] and result['bytes_without_term'], 'Claude printed nothing; gate inconclusive'
    assert not result['colored_without_term'], 'control run is colored; gate cannot tell the cases apart'
    assert result['colored_with_term'], 'wrapper drops TERM/COLORTERM: Claude renders monochrome'

def live(root):
    mcp = str(pathlib.Path(root).resolve()/'mcp.json')
    pids = subprocess.run(['pgrep', '-f', '--', '--mcp-config '+mcp], capture_output=True, text=True).stdout.split()
    assert pids, 'no Claude process under this acceptance root'
    rows = []
    for pid in pids:
        words = subprocess.run(['ps', '-E', '-ww', '-o', 'command=', '-p', pid], capture_output=True, text=True).stdout.split()
        # Report names only for anything but the two terminal variables; never values.
        rows.append({'pid': int(pid), **{k: next((w.split('=', 1)[1] for w in words if w.startswith(k+'=')), None) for k in ('TERM', 'COLORTERM')}})
    print(json.dumps(rows))
    assert all(r['TERM'] and r['COLORTERM'] for r in rows), 'Claude process lacks TERM/COLORTERM'

if __name__ == '__main__':
    live(sys.argv[2]) if sys.argv[1:2] == ['--live'] else offline()
