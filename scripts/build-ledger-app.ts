import { mkdirSync, copyFileSync } from 'fs';
import { join } from 'path';
const root = join(import.meta.dir,'..');
for (const command of [['bun','run','build:client'],['bun','run','build:embedded-service']]) {
  const process = Bun.spawn(command,{ cwd: root,stdout: 'inherit',stderr: 'inherit' });
  if (await process.exited !== 0) throw new Error(`${command.join(' ')} failed`);
}
const target = Bun.spawn(['rustc','-vV'],{ stdout: 'pipe' });
const version = await new Response(target.stdout).text();
if (await target.exited !== 0) throw new Error('rustc target detection failed');
const triple = version.match(/^host: (.+)$/m)?.[1]; if (!triple) throw new Error('Missing rustc host triple');
const directory = join(root,'menubar-tauri','src-tauri','binaries'); mkdirSync(directory,{ recursive: true });
copyFileSync(join(root,'dist','keepline-service'),join(directory,`keepline-service-${triple}`));
