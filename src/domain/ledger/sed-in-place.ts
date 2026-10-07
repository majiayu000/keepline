/** Shell words for direct commands; expansions and control flow remain uncertain. */
export function directWords(command: string): string[] | undefined {
  const words: string[] = [];
  let word = '', quote: "'" | '"' | undefined, started = false;
  for (let index = 0; index < command.length; index++) {
    const char = command[index];
    if (char === '\\' && quote !== "'") {
      const next = command[index + 1];
      if (next === undefined) return undefined;
      if (quote === '"' && !['"', '\\', '$', '`', '\n'].includes(next)) word += char;
      else { if (next !== '\n') word += next; index++; }
      started = true;
    } else if (quote) {
      if (char === quote) quote = undefined;
      else if (quote === '"' && /[$`]/.test(char)) return undefined;
      else word += char;
    } else if (char === "'" || char === '"') {
      quote = char; started = true;
    } else if (char === '#' && !started) {
      const newline = command.indexOf('\n', index);
      if (newline >= 0 && command.slice(newline + 1).trim()) return undefined;
      break;
    } else if (char === ' ' || char === '\t') {
      if (started) { words.push(word); word = ''; started = false; }
    } else {
      if (/[\r\n;&|<>`$()]/.test(char)) return undefined;
      word += char; started = true;
    }
  }
  if (quote) return undefined;
  if (started) words.push(word);
  return words;
}

/** File operands of a direct in-place edit; expansions/control flow stay unknown. */
function sedCommand(command: unknown) {
  if (typeof command !== 'string') return undefined;
  const words = directWords(command);
  if (words?.[0] !== 'sed') return undefined;
  const files: string[] = [];
  let inPlace = false, hasScript = false, options = true, ambiguousSuffix = false, scriptFile = false;
  const scripts: string[] = [];
  for (let index = 1; index < words.length; index++) {
    const option = words[index];
    if (options && option === '--') { options = false; continue; }
    if (!options || !option.startsWith('-') || option === '-') {
      if (!hasScript) { hasScript = true; scripts.push(option); }
      else files.push(option);
      continue;
    }
    // These options exit before editing; after -- they are ordinary file names.
    if (option === '--help' || option === '--version') return undefined;
    if (option === '--in-place' || option.startsWith('--in-place=')) { inPlace = true; continue; }
    if (option === '--expression' || option === '--file') {
      hasScript = true;
      if (option === '--file') scriptFile = true;
      else scripts.push(words[index + 1] ?? '');
      index++; continue;
    }
    if (option.startsWith('--expression=') || option.startsWith('--file=')) {
      hasScript = true;
      if (option.startsWith('--file=')) scriptFile = true;
      else scripts.push(option.slice('--expression='.length));
      continue;
    }
    if (option.startsWith('--')) continue;
    for (let flag = 1; flag < option.length; flag++) {
      if (option[flag] === 'i') {
        inPlace = true;
        // Bare -i consumes a suffix on BSD but not GNU. Without host
        // provenance, retain the mutation attempt and avoid guessing files.
        if (flag + 1 === option.length) {
          if (words[index + 1] === '') index++;
          else ambiguousSuffix = true;
        }
        break;
      }
      if (option[flag] === 'e' || option[flag] === 'f') {
        hasScript = true;
        // -e/-f consume the rest of their word, or the next complete shell word.
        if (option[flag] === 'f') scriptFile = true;
        else scripts.push(option.slice(flag + 1) || words[index + 1] || '');
        if (flag + 1 === option.length) index++;
        break;
      }
    }
  }
  return { inPlace, files: ambiguousSuffix ? [] : files, scripts, scriptFile };
}

export function sedInPlaceFiles(command: unknown): string[] | undefined {
  const parsed = sedCommand(command);
  return parsed?.inPlace ? parsed.files : undefined;
}

// Skip regular-expression and replacement fields before inspecting script commands.
// Unknown external scripts can write; their contents are absent from the receipt.
function scriptMayWrite(script: string): boolean {
  for (let index = 0; index < script.length; index++) {
    const char = script[index];
    if (char === '\\') { index++; continue; }
    if (char === '#' || /[rRbBtT:]/.test(char)) {
      while (index + 1 < script.length && !/[;\n]/.test(script[index + 1])) index++;
      continue;
    }
    if (char === 'w' || char === 'W' || char === 'e') return true;
    const fields = char === 's' || char === 'y' ? 2 : char === '/' ? 1 : 0;
    if (!fields) continue;
    const delimiter = char === '/' ? '/' : script[++index];
    if (!delimiter) return true;
    for (let field = 0; field < fields; field++) {
      let closed = false;
      while (++index < script.length) {
        if (script[index] === '\\') index++;
        else if (script[index] === delimiter) { closed = true; break; }
      }
      if (!closed) return true;
    }
  }
  return false;
}

export function isSedWriteCommand(command: unknown): boolean {
  const parsed = sedCommand(command);
  return !!parsed && (parsed.inPlace || parsed.scriptFile || parsed.scripts.some(scriptMayWrite));
}

/** Identify sed's in-place option, never option-like text in an expression or file name. */
export function isSedInPlaceCommand(command: unknown): boolean {
  return sedInPlaceFiles(command) !== undefined;
}
