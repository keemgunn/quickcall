import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it } from "vitest";
import { piArgs, runPi } from "../../src/pi.js";
import { removeTemp, tempDir } from "../helpers/temp.js";
const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../.."); const fixtureBin = join(packageRoot, "tests/fixtures/bin"); const clean: string[] = []; afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));
it("constructs deterministic argv for omitted, true, false, and overridden settings", () => {
  expect(piArgs({})).toEqual(["-p"]); expect(piArgs({ noSkills: false, approve: true }, "global", "medium")).toEqual(["-p", "--model", "global", "--thinking", "medium", "--approve"]); expect(piArgs({ model: "m", thinking: "high", noSkills: true, skillPath: "/skills", approve: false })).toEqual(["-p", "--model", "m", "--thinking", "high", "--no-skills", "--skill", "/skills", "--no-approve"]);
});
it.each([["SIGINT", 130], ["SIGTERM", 143]] as const)("maps Pi %s termination to exit %i", async (signal, exit) => {
  const cwd = await tempDir("pi-signal"); clean.push(cwd);
  await expect(runPi("prompt", ["-p"], cwd, { ...process.env, PATH: `${fixtureBin}:${process.env.PATH}`, QC_PI_SIGNAL: signal })).resolves.toBe(exit);
});
