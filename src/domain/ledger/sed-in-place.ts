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

/** File operands of a direct in-place edit; expansions/control flow stay unknown. */
export function sedInPlaceFiles(command: unknown): string[] | undefined {
  if (typeof command !== 'string') return undefined;
  const words = directWords(command);
  if (words?.[0] !== 'sed') return undefined;
  const files: string[] = [];
  let inPlace = false, hasScript = false, options = true;
  for (let index = 1; index < words.length; index++) {
    const option = words[index];
    if (options && option === '--') { options = false; continue; }
    if (!options || !option.startsWith('-') || option === '-') {
      if (!hasScript) hasScript = true;
      else files.push(option);
      continue;
    }
    if (option === '--in-place' || option.startsWith('--in-place=')) { inPlace = true; continue; }
    if (option === '--expression' || option === '--file') { hasScript = true; index++; continue; }
    if (option.startsWith('--expression=') || option.startsWith('--file=')) { hasScript = true; continue; }
    if (option.startsWith('--')) continue;
    for (let flag = 1; flag < option.length; flag++) {
      if (option[flag] === 'i') {
        inPlace = true;
        // BSD sed accepts an empty backup suffix as a separate shell word.
        if (flag + 1 === option.length && words[index + 1] === '') index++;
        break;
      }
      if (option[flag] === 'e' || option[flag] === 'f') {
        hasScript = true;
        // -e/-f consume the rest of their word, or the next complete shell word.
        if (flag + 1 === option.length) index++;
        break;
      }
    }
  }
  return inPlace ? files : undefined;
}

/** Identify sed's in-place option, never option-like text in an expression or file name. */
export function isSedInPlaceCommand(command: unknown): boolean {
  return sedInPlaceFiles(command) !== undefined;
}
