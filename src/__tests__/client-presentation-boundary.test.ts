import { test, expect } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

test("ledger presentation delegates HTTP operations and WebSocket state to hooks", () => {
  const root = join(import.meta.dir, "../web/client/src/pages/ledger");
  const files = [
    join(root, "LedgerPage.tsx"),
    ...readdirSync(join(root, "components"))
      .filter((name) => name.endsWith(".tsx"))
      .map((name) => join(root, "components", name)),
  ];
  for (const path of files) {
    const source = readFileSync(path, "utf8");
    expect(
      source,
      `${path} must delegate requests to the controller`,
    ).not.toMatch(/from\s+['"]@\/services\/(api|ledger|websocket)['"]/);
    expect(source, `${path} must not perform HTTP requests`).not.toMatch(
      /\bfetch\s*\(/,
    );
  }
});
