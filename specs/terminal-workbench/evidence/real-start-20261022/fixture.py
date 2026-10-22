"""Private real-CLI acceptance. Provider secret is read only inside exec wrapper."""
import json, os, pathlib, plistlib, runpy, shlex, shutil, subprocess, tempfile, time
OUT = pathlib.Path(__file__).resolve().parent
BASE = OUT.parents[3]
m = runpy.run_path(str(OUT.parent/'r4-20261021/native-fixture.py'))
rpc, edit = m['rpc'], m['edit']
if __name__ == '__main__':
    root = pathlib.Path(tempfile.mkdtemp(prefix='kl-start22-', dir='/tmp'))
    root.chmod(0o700)
    home = root/'home'; home.mkdir(mode=0o700)
    for p in [home/'.codex/sessions', home/'.claude/projects', root/'bin', root/'project']:
        p.mkdir(parents=True, exist_ok=True)
    # No auth initially: first check the genuine Codex login screen before adding
    # the authorized isolated Codex auth copy. User config is never modified.
    (home/'.codex/config.toml').write_text('model = "gpt-6.1-sol"\nmodel_reasoning_effort = "low"\ncheck_for_update_on_startup = false\n')
    (home/'.claude/settings.json').write_text(json.dumps({'disableAllHooks':True}))
    (home/'.claude.json').write_text(json.dumps({'hasCompletedOnboarding':True,'theme':'dark'}))
    (root/'mcp.json').write_text('{"mcpServers":{}}')
    # Reuse only the secret-free exec-wrapper source; don't run the old fixture.
    import ast
    tree=ast.parse((OUT.parent/'r9-20261021/fixture.py').read_text())
    assignment=next(n for n in ast.walk(tree) if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='script' for t in n.targets))
    template=assignment.value.func.value.func.value.value
    assert isinstance(template,str) and 'settings_config' in template
    script=template.replace('DB',repr(str(pathlib.Path.home()/'.cc-switch/cc-switch.db'))).replace('CLI',repr(str(pathlib.Path.home()/'.local/bin/claude')))
    (root/'bin/claude').write_text(script); (root/'bin/claude').chmod(0o700)
    (root/'bin/codex').symlink_to('/opt/homebrew/bin/codex')
    env={'PATH':str(root/'bin')+':/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin','HOME':str(home),'CODEX_HOME':str(home/'.codex'),'CLAUDE_CONFIG_DIR':str(home/'.claude'),'LANG':'en_US.UTF-8','SHELL':'/bin/zsh'}
    b=root/'Keepline Start Acceptance.app'; mac=b/'Contents/MacOS'; mac.mkdir(parents=True)
    for n in ['keepline-app','keeplined']:shutil.copy2(BASE/'next/target/release'/n,mac/n)
    launcher=mac/'launch';launcher.write_text('#!/bin/sh\nexec /usr/bin/env -i '+' '.join(k+'='+shlex.quote(v) for k,v in env.items())+' '+shlex.quote(str(mac/'keepline-app'))+' --runtime '+shlex.quote(str(root/'runtime'))+' >>'+shlex.quote(str(root/'client.log'))+' 2>&1\n');launcher.chmod(0o700)
    (b/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleName':'Keepline Start Acceptance','CFBundleDisplayName':'Keepline Start Acceptance','CFBundleIdentifier':'local.keepline.start-acceptance22','CFBundleExecutable':'launch','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True}))
    subprocess.run(['codesign','--force','--sign','-',str(b)],check=True,capture_output=True)
    host=subprocess.Popen([str(mac/'keeplined'),'serve','--runtime',str(root/'runtime')],env=env,stdout=subprocess.DEVNULL,stderr=open(root/'host.log','w'),start_new_session=True)
    end=time.monotonic()+30
    while not (root/'runtime/workbench.sock').exists():
        assert host.poll() is None and time.monotonic()<end
        time.sleep(.1)
    edit(root,{'AddSpace':{'id':'accept','name':'真实启动隔离验收','cwd':str(root/'project')}})
    for agent in ['Codex','Claude']:
        edit(root,{'AddTask':{'id':agent.lower(),'space':'accept','title':f'请创建 {agent.lower()}-proof.txt，内容仅为 ok','description':'只使用一次文件写入工具，不读其他文件、不调用网络。完成后只回复：完成。','acceptance':[{'id':'file','description':'文件内容为 ok','evidence':None}],'agent':agent}})
    state={'root':str(root),'home':str(home),'bundle':str(b),'host':host.pid,'project':str(root/'project')}
    pathlib.Path('/tmp/kl-start22.json').write_text(json.dumps(state)); print(json.dumps(state))
