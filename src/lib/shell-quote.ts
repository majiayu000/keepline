/**
 * Shell argument rendering for terminal display/automation.
 */

const SAFE_SHELL_WORD = /^[A-Za-z0-9_@%+=:,./-]+$/;

/** Quote exactly one shell argument. */
export function shellQuote(value: string): string {
  if (value.length === 0) {
    return "''";
  }

  if (SAFE_SHELL_WORD.test(value)) {
    return value;
  }

  return `'${value.replace(/'/g, "'\\''")}'`;
}

/** Render argv as a shell command string for terminal apps. */
export function renderShellCommand(args: readonly string[]): string {
  if (args.length === 0) {
    throw new Error('Cannot render empty shell command');
  }

  return args.map(shellQuote).join(' ');
}
