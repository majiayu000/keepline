import { execFile } from 'child_process';

export function macOSNotification(message: string, sound = false): Promise<void> {
  if (process.platform !== 'darwin') return Promise.resolve();
  const script = 'on run argv\n display notification (item 1 of argv) with title "Keepline"' + (sound ? ' sound name "Glass"' : '') + '\nend run';
  return new Promise((resolve,reject) => execFile('osascript',['-e',script,message],{ timeout: 5000 },error => error ? reject(error) : resolve()));
}
