/** Shell words for direct commands; expansions and control flow remain uncertain. */
function directWords(command: string): string[] | undefined {
  if (/[\r\n;&|<>`$]/.test(command)) return undefined;
  const words: string[] = [];
  let word = '', quote: "'" | '"' | undefined, started = false;
  for (let index = 0; index < command.length; index++) {
    const char = command[index];
    if (char === '\\' && quote !== "'") {
      const next = command[index + 1];
      if (next === undefined) return undefined;
      if (quote === '"' && next !== '"' && next !== '\\') word += char;
      else { word += next; index++; }
      started = true;
    } else if (quote) {
      if (char === quote) quote = undefined;
      else word += char;
    } else if (char === "'" || char === '"') {
      quote = char; started = true;
    } else if (char === '#' && !started) {
      break;
    } else if (char === ' ' || char === '\t') {
      if (started) { words.push(word); word = ''; started = false; }
    } else {
      word += char; started = true;
    }
  }
  if (quote) return undefined;
  if (started) words.push(word);
  return words;
}

/** Identify sed's in-place option, never option-like text in an expression or file name. */
export function isSedInPlaceCommand(command: unknown): boolean {
  if (typeof command !== 'string') return false;
  const words = directWords(command);
  if (words?.[0] !== 'sed') return false;
  for (let index = 1; index < words.length; index++) {
    const option = words[index];
    if (option === '--') break;
    if (option === '--in-place' || option.startsWith('--in-place=')) return true;
    if (option === '--expression' || option === '--file') { index++; continue; }
    if (!option.startsWith('-') || option.startsWith('--')) continue;
    for (let flag = 1; flag < option.length; flag++) {
      if (option[flag] === 'i') return true;
      if (option[flag] === 'e' || option[flag] === 'f') {
        // -e/-f consume the rest of their word, or the next complete shell word.
        if (flag + 1 === option.length) index++;
        break;
      }
    }
  }
  return false;
}
