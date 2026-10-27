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
parser.add_argument('--output', type=pathlib.Path, default=root / 'target/bundle/Keepline.app',
                    help='Bundle path; use a separate path when an installed build is running')
parser.add_argument('--icon', type=pathlib.Path,
                    default=root.parent / 'docs/assets/keepline-icon-final/Keepline.icns',
                    help='App icon (.icns); defaults to the ink-ring icon')
# A stable identity keeps macOS privacy grants (e.g. Desktop access) across rebuilds;
# ad-hoc ('-') signatures change with every build, so macOS asks again each time.
parser.add_argument('--sign-identity', default='Developer ID Application: Ma JiaYu (C5UWZ934C2)',
                    help="codesign identity; pass '-' for ad-hoc signing")
args = parser.parse_args()
bundle = args.output.resolve()
sign = ['codesign', '--force', '--sign', args.sign_identity]
if args.sign_identity != '-':
    # Rust/GPUI uses no JIT or unsigned plug-ins. No hardened-runtime exceptions needed.
    sign += ['--options', 'runtime', '--timestamp']
mac = bundle / 'Contents/MacOS'
resources = bundle / 'Contents/Resources'
mac.mkdir(parents=True, exist_ok=True)
resources.mkdir(parents=True, exist_ok=True)
for binary in ('keepline-app', 'keeplined'):
    shutil.copy2(args.release_dir / binary, mac / binary)
    subprocess.run([*sign, str(mac / binary)], check=True)
shutil.copy2(root / 'crates/keepline-app/assets/NERD-FONTS-LICENSE', resources / 'NERD-FONTS-LICENSE')
shutil.copy2(args.icon, resources / 'Keepline.icns')
(bundle / 'Contents/Info.plist').write_bytes(plistlib.dumps({
    'CFBundleName': 'Keepline', 'CFBundleDisplayName': 'Keepline',
    'CFBundleIdentifier': 'local.keepline.workbench', 'CFBundleExecutable': 'keepline-app',
    'CFBundlePackageType': 'APPL', 'CFBundleIconFile': 'Keepline', 'CFBundleVersion': '1', 'CFBundleShortVersionString': '0.1.0',
    'NSHighResolutionCapable': True, 'LSMinimumSystemVersion': '14.0',
}))
subprocess.run([*sign, str(bundle)], check=True)
subprocess.run(['codesign', '--verify', '--deep', '--strict', str(bundle)], check=True)
receipt = {'bundle': str(bundle), 'signature': args.sign_identity,
           'hardened_runtime': args.sign_identity != '-', 'notarized': False, 'published': False,
           'git_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
           'binaries': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in mac.iterdir() if p.is_file()}}
(bundle.parent / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
