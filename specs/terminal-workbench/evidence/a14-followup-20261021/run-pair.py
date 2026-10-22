"""Idle-gated foreground audit; preserve all attempts, never retry valid slow samples.
Compile frontmost.swift to /tmp/kl-a14-idle-guard first. cases.json contains only
explicit isolated test PIDs; recreate those fixtures before running on another day.
"""
import json
import pathlib
import subprocess
import time

out = pathlib.Path(__file__).resolve().parent
state = []
for case in json.loads((out / 'cases.json').read_text()):
    name = case['name']
    specpath = out / case['spec']
    spec = json.loads(specpath.read_text())
    for attempt in range(1, 4):
        prefix = out / f'{name}-batch-{attempt}'
        if any(pathlib.Path(str(prefix) + suffix).exists() for suffix in ('-focus-1.json', '-idle-gate.json')):
            raise RuntimeError(f'Refusing to overwrite {prefix}')
        (out / 'progress.json').write_text(json.dumps({
            'case': name, 'attempt': attempt,
            'phase': 'waiting-for-input-idle-or-sampling', 'time': time.time()}))
        cmd = ['/tmp/kl-a14-idle-guard', str(spec['pids']['client']), str(prefix),
               '/usr/bin/python3', str(out.parent / 'r5-20261021/measure.py'),
               str(specpath), str(prefix) + '.json']
        with open(str(prefix) + '.txt', 'w') as log:
            result = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, timeout=240)
        focus = pathlib.Path(str(prefix) + '-focus-1.json')
        valid = result.returncode == 0 and focus.exists() and json.loads(focus.read_text())['continuous_frontmost']
        state.append({'case': name, 'attempt': attempt, 'valid': valid,
                      'exit': result.returncode, 'prefix': prefix.name})
        (out / 'attempts.json').write_text(json.dumps(state, indent=2) + '\n')
        print(state[-1], flush=True)
        if valid or result.returncode == 2:
            break
    if result.returncode == 2:
        # Input-idle is a shared prerequisite, not four independent failures.
        break
(out / 'progress.json').write_text(json.dumps({'phase': 'blocked' if result.returncode == 2 else 'complete', 'time': time.time()}))
