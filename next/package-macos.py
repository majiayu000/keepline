#!/usr/bin/env python3
"""Package the current release build locally; no installation, notarization or publishing."""
import argparse
import hashlib
import json
import pathlib
import plistlib
import shutil
import subprocess

root = pathlib.Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--release-dir', type=pathlib.Path, default=root / 'target/release')
args = parser.parse_args()
bundle = root / 'target/bundle/Keepline.app'
mac = bundle / 'Contents/MacOS'
resources = bundle / 'Contents/Resources'
mac.mkdir(parents=True, exist_ok=True)
resources.mkdir(parents=True, exist_ok=True)
for binary in ('keepline-app', 'keeplined'):
    shutil.copy2(args.release_dir / binary, mac / binary)
    subprocess.run(['codesign', '--force', '--sign', '-', str(mac / binary)], check=True)
shutil.copy2(root / 'crates/keepline-app/assets/NERD-FONTS-LICENSE', resources / 'NERD-FONTS-LICENSE')
(bundle / 'Contents/Info.plist').write_bytes(plistlib.dumps({
    'CFBundleName': 'Keepline', 'CFBundleDisplayName': 'Keepline',
    'CFBundleIdentifier': 'local.keepline.workbench', 'CFBundleExecutable': 'keepline-app',
    'CFBundlePackageType': 'APPL', 'CFBundleVersion': '1', 'CFBundleShortVersionString': '0.1.0',
    'NSHighResolutionCapable': True, 'LSMinimumSystemVersion': '14.0',
}))
subprocess.run(['codesign', '--force', '--sign', '-', str(bundle)], check=True)
subprocess.run(['codesign', '--verify', '--deep', '--strict', str(bundle)], check=True)
receipt = {'bundle': str(bundle), 'signature': 'ad-hoc; not notarized or published',
           'git_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
           'binaries': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in mac.iterdir() if p.is_file()}}
(bundle.parent / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
