import { describe, expect, it } from "vitest";
import { downgradeRunes } from "../src/core/syntax.ts";

describe("downgradeRunes", () => {
  it("rewrites a root declaration into markRoot", () => {
    const result = downgradeRunes("rune user = loadUser();", "src/main.ts");
    expect(result.roots).toEqual([{ name: "user", line: 1, column: 1 }]);
    expect(result.source).toBe(
      'const user = __runes.markRoot("user", loadUser(), ' + '"src/main.ts", 1, 1);',
    );
  });

  it("keeps semicolons inside strings and brackets", () => {
    const result = downgradeRunes("rune cfg = { sep: ';', list: [1, 2] };", "a.ts");
    expect(result.source).toBe(
      'const cfg = __runes.markRoot("cfg", ' + "{ sep: ';', list: [1, 2] }, \"a.ts\", 1, 1);",
    );
  });

  it("ignores the keyword inside comments and strings", () => {
    const source = '// rune fake = 1;\nconst s = "rune alsoFake = 2;";\n';
    expect(downgradeRunes(source, "a.ts").roots).toEqual([]);
  });

  it("supports a custom keyword", () => {
    const result = downgradeRunes("trace x = 1;", "a.ts", "trace");
    expect(result.roots[0]?.name).toBe("x");
  });

  it("records line and column of the keyword", () => {
    const result = downgradeRunes("\n\n  rune user = f();", "a.ts");
    expect(result.roots[0]).toEqual({ name: "user", line: 3, column: 3 });
  });

  it("throws on a malformed declaration", () => {
    expect(() => downgradeRunes("rune = 1;", "a.ts")).toThrow();
  });
});
