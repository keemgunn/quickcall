import { readFile } from "node:fs/promises";
import { expect, it } from "vitest";
import { error, help, version } from "../../src/messages.js";

it("centralizes manifest-derived user-facing output", async () => {
  const manifest = JSON.parse(await readFile(new URL("../../package.json", import.meta.url), "utf8")) as { version: string };
  expect(help).toContain("Usage: qc"); expect(help).toContain("-s, --shell"); expect(help).toContain("-a, --append");
  expect(help).toContain("--install-sample-prompts"); expect(help).toContain("--install-agent-harness");
  expect(help).toContain("-h, --help"); expect(help).toContain("-v, --version"); expect(error("bad")).toBe("qc: error: bad"); expect(version).toBe(manifest.version);
});
