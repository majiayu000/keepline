"""Dedicated Ghostty 1.3.1 copy with the R5/R8 config: `ghostty-fixture.py <1|4> <label>`.
Only the copy's bundle ID and ad-hoc signature change; /Applications and user config are untouched.
Four panes: the first surface asks its own app (AppleScript to itself, no other app is
controlled) for a 2x2 split, then every surface runs the same R5 load.py."""
import json, pathlib, plistlib, shlex, shutil, subprocess, sys, tempfile, time
out = pathlib.Path(__file__).resolve().parent; ev = out.parent
panes, label = int(sys.argv[1]), sys.argv[2]
r = pathlib.Path(tempfile.mkdtemp(prefix=f'kl14-{label}-', dir='/tmp'))
app = r / f'Ghostty A14 {label}.app'; bid = f'local.keepline.a14.{label}'
subprocess.run(['ditto', '/Applications/Ghostty.app', str(app)], check=True)
info = app / 'Contents/Info.plist'; p = plistlib.loads(info.read_bytes()); p['CFBundleIdentifier'] = bid
info.write_bytes(plistlib.dumps(p))
subprocess.run(['codesign', '--force', '--deep', '--sign', '-', str(app)], check=True, capture_output=True)
load = ev / ('r8-20261021/load-one.py' if panes == 1 else 'r5-20261021/load.py')
command = f'/usr/bin/python3 {load}'
if panes == 4:
    # mkdir is atomic: only the first surface creates the splits; the others just run the load.
    t = 'terminal 1 of selected tab of front window'
    split = (f'tell application id "{bid}"\n set a to {t}\n set b to split a direction right\n'
             f' split a direction down\n split b direction down\nend tell\n')
    (r / 'split.applescript').write_text(split)
    (r / 'surface.sh').write_text('#!/bin/sh\nif mkdir ' + shlex.quote(str(r / 'split.lock')) + ' 2>/dev/null; then\n'
                                  ' (sleep 1; /usr/bin/osascript ' + shlex.quote(str(r / 'split.applescript'))
                                  + ' >' + shlex.quote(str(r / 'split.log')) + ' 2>&1) &\nfi\nexec ' + command + '\n')
    (r / 'surface.sh').chmod(0o755); command = str(r / 'surface.sh')
config = r / 'config'
config.write_text('\n'.join([
    'font-family = Menlo', 'font-size = 12.5', 'font-thicken = false', 'font-feature = -calt', 'font-feature = -liga',
    'window-width = 160', 'window-height = 48', 'window-padding-x = 0', 'window-padding-y = 0',
    'window-save-state = never', 'window-step-resize = false', 'background = 101217', 'foreground = d7dae0',
    'cursor-style-blink = false', 'shell-integration = none', 'confirm-close-surface = false',
    'quit-after-last-window-closed = true', f'command = {command}']) + '\n')
# Same launch as earlier copies: the user's own Ghostty config holds only a theme, whose
# colors the --config-file overrides. (--config-default-files=false / --env HOME left the
# copy without any surface on this machine, so they are not used.)
subprocess.run(['open', '-n', '--stdout', str(r / 'stdout.log'), '--stderr', str(r / 'stderr.log'), str(app),
                '--args', f'--config-file={config}'], check=True)
exe = str(app / 'Contents/MacOS/ghostty'); client = None; end = time.monotonic() + 20
while client is None:
    assert time.monotonic() < end; time.sleep(.3)
    ps = subprocess.run(['pgrep', '-f', exe], capture_output=True, text=True).stdout.split()
    client = int(ps[0]) if ps else None
def kids(pid):
    return [int(x) for x in subprocess.run(['pgrep', '-P', str(pid)], capture_output=True, text=True).stdout.split()]
end = time.monotonic() + 30
while True:
    # Ghostty: client -> login wrapper -> python producer (surface.sh execs python).
    logins = kids(client); producers = [k for l in logins for k in kids(l)
               if str(load) in subprocess.run(['ps', '-o', 'command=', '-p', str(k)], capture_output=True, text=True).stdout]
    if len(producers) == panes: break
    assert time.monotonic() < end, (logins, producers); time.sleep(.5)
pids = {'client': client, **{f'producer{i+1}': p for i, p in enumerate(sorted(producers))}}
spec = {'label': label, 'root': str(r), 'pids': pids, 'producers': sorted(producers),
        'excluded_login_wrappers': sorted(logins)}
(out / f'{label}-spec.json').write_text(json.dumps(spec, indent=2) + '\n'); print(json.dumps(spec))
