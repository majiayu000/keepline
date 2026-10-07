import { mkdirSync, copyFileSync } from 'fs';
import { join } from 'path';
const root = join(import.meta.dir,'..');
const target = Bun.spawn(['rustc','-vV'],{ stdout: 'pipe' });
const version = await new Response(target.stdout).text();
if (await target.exited !== 0) throw new Error('rustc target detection failed');
const triple = process.env.KEEPLINE_BUILD_TARGET ?? version.match(/^host: (.+)$/m)?.[1];
const bunTarget = triple === 'aarch64-apple-darwin' ? 'bun-darwin-arm64' : triple === 'x86_64-apple-darwin' ? 'bun-darwin-x64' : undefined;
if (!bunTarget) throw new Error(`Unsupported desktop target: ${triple}`);
for (const command of [['bun','run','build:client'],['bun','build','--compile',`--target=${bunTarget}`,'src/embedded-service.ts','--outfile','dist/keepline-service']]) {
  const process = Bun.spawn(command,{ cwd: root,stdout: 'inherit',stderr: 'inherit' });
  if (await process.exited !== 0) throw new Error(`${command.join(' ')} failed`);
}
const directory = join(root,'menubar-tauri','src-tauri','binaries'); mkdirSync(directory,{ recursive: true });
copyFileSync(join(root,'dist','keepline-service'),join(directory,`keepline-service-${triple}`));
