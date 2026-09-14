/**
 * Text-level downgrade of `rune` root declarations.
 *
 * \`rune name = expr;\` is not valid TypeScript, so — mirroring the Rust
 * driver's pre-parse lowering — we rewrite it before the compiler sees it:
 *
 *     rune user = loadUser();
 *     // becomes
 *     const user = __runes.markRoot("user", loadUser(), "src/main.ts", 3, 7);
 *
 * The scanner is string/comment aware and only treats the keyword at a
 * statement position (identifier boundary, followed by a binding name and
 * \`=\`) as a root declaration.
 */

/** A root declaration found during downgrade. */
export interface RuneRoot {
  /** Binding name being marked as a trace root. */
  name: string;
  /** 1-based line of the keyword in the original source. */
  line: number;
  /** 1-based column of the keyword in the original source. */
  column: number;
}

/** Result of downgrading root declarations in a source file. */
export interface DowngradeResult {
  /** Valid TypeScript source with __runes.markRoot calls inlined. */
  source: string;
  /** All roots declared in this file, in source order. */
  roots: RuneRoot[];
}

const DEFAULT_KEYWORD = "rune";
const BACKTICK = String.fromCharCode(96);

function isIdentStart(ch: string): boolean {
  return /[A-Za-z_$]/.test(ch);
}

function isIdentChar(ch: string): boolean {
  return /[A-Za-z0-9_$]/.test(ch);
}

/** Computes the 1-based line/column of index within text. */
export function lineColumnAt(text: string, index: number): { line: number; column: number } {
  let line = 1;
  let lastBreak = -1;
  for (let i = 0; i < index; i++) {
    if (text[i] === "\n") {
      line++;
      lastBreak = i;
    }
  }
  return { line, column: index - lastBreak };
}

/**
 * Rewrites every root declaration in source into a const binding wrapped
 * in __runes.markRoot(...).
 *
 * Throws when a declaration is malformed (missing binding name, missing
 * '=', or an unterminated expression).
 */
export function downgradeRunes(
  source: string,
  fileName: string,
  keyword: string = DEFAULT_KEYWORD,
): DowngradeResult {
  const roots: RuneRoot[] = [];
  const chunks: string[] = [];
  let cursor = 0;
  let i = 0;
  const n = source.length;

  function readIdent(pos: number): { text: string; end: number } | null {
    if (pos >= n || !isIdentStart(source[pos])) return null;
    let end = pos + 1;
    while (end < n && isIdentChar(source[end])) end++;
    return { text: source.slice(pos, end), end };
  }

  function skipWs(pos: number): number {
    while (pos < n && /\s/.test(source[pos])) pos++;
    return pos;
  }

  // Returns the index just past the string literal starting at pos.
  function skipString(pos: number, quote: string): number {
    let j = pos + 1;
    while (j < n) {
      const ch = source[j];
      if (ch === "\\") {
        j += 2;
        continue;
      }
      if (ch === quote) return j + 1;
      if (quote === BACKTICK && ch === "$" && source[j + 1] === "{") {
        let depth = 1;
        j += 2;
        while (j < n && depth > 0) {
          const c = source[j];
          if (c === "'" || c === '"' || c === BACKTICK) {
            j = skipString(j, c);
            continue;
          }
          if (c === "{") depth++;
          else if (c === "}") depth--;
          j++;
        }
        continue;
      }
      j++;
    }
    throw new Error(fileName + ": unterminated string literal");
  }

  // Finds the index of the ';' terminating the expression starting at
  // pos, tracking bracket depth and string/comment states.
  function findStatementEnd(pos: number): number {
    let depth = 0;
    let j = pos;
    while (j < n) {
      const ch = source[j];
      const next = source[j + 1];
      if (ch === "/" && next === "/") {
        const nl = source.indexOf("\n", j);
        j = nl === -1 ? n : nl;
        continue;
      }
      if (ch === "/" && next === "*") {
        const close = source.indexOf("*/", j + 2);
        if (close === -1) throw new Error(fileName + ": unterminated comment");
        j = close + 2;
        continue;
      }
      if (ch === "'" || ch === '"' || ch === BACKTICK) {
        j = skipString(j, ch);
        continue;
      }
      if (ch === "(" || ch === "[" || ch === "{") depth++;
      else if (ch === ")" || ch === "]" || ch === "}") depth--;
      else if (ch === ";" && depth === 0) return j;
      j++;
    }
    throw new Error(fileName + ": unterminated " + keyword + " declaration");
  }

  while (i < n) {
    const ch = source[i];
    const next = source[i + 1];
    if (ch === "/" && next === "/") {
      const nl = source.indexOf("\n", i);
      i = nl === -1 ? n : nl;
      continue;
    }
    if (ch === "/" && next === "*") {
      const close = source.indexOf("*/", i + 2);
      if (close === -1) throw new Error(fileName + ": unterminated comment");
      i = close + 2;
      continue;
    }
    if (ch === "'" || ch === '"' || ch === BACKTICK) {
      i = skipString(i, ch);
      continue;
    }

    if (isIdentStart(ch)) {
      const ident = readIdent(i);
      if (ident && ident.text === keyword) {
        const name = readIdent(skipWs(ident.end));
        if (!name) {
          throw new Error(fileName + ": " + keyword + " must be followed by a binding name");
        }
        const eq = skipWs(name.end);
        if (source[eq] !== "=" || source[eq + 1] === "=") {
          throw new Error(
            fileName + ": " + keyword + " " + name.text + " must be followed by = <expression>;",
          );
        }
        const exprStart = skipWs(eq + 1);
        const end = findStatementEnd(exprStart);
        const expr = source.slice(exprStart, end);
        const pos = lineColumnAt(source, i);
        roots.push({ name: name.text, line: pos.line, column: pos.column });
        chunks.push(source.slice(cursor, i));
        chunks.push(
          "const " +
            name.text +
            " = __runes.markRoot(" +
            JSON.stringify(name.text) +
            ", " +
            expr +
            ", " +
            JSON.stringify(fileName) +
            ", " +
            pos.line +
            ", " +
            pos.column +
            ");",
        );
        cursor = end + 1;
        i = end + 1;
        continue;
      }
      if (ident) {
        i = ident.end;
        continue;
      }
    }
    i++;
  }

  if (roots.length === 0) return { source, roots };
  chunks.push(source.slice(cursor));
  return { source: chunks.join(""), roots };
}
