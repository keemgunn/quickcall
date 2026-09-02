import { join } from "node:path";
import { afterEach, expect, it } from "vitest";
import { expandShell, expressions } from "../../src/shell-output.js";
import { removeTemp, tempDir } from "../helpers/temp.js";
const clean: string[] = [];
afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));
async function shellContext() {
  const cwd = await tempDir("shell-output"); clean.push(cwd);
  return { cwd, environment: { HOME: join(cwd, "home"), PATH: "/usr/bin:/bin", SHELL: "/bin/sh" } };
}
it("executes substitutions concurrently and replaces in source order", async () => {
  const { cwd, environment } = await shellContext(); const result = await expandShell("!`sleep 0.05; printf first` !`printf second`", "/bin/sh", { rules: [], hasPermissions: false }, cwd, environment);
  expect(result).toBe("first second");
});
it("validates the selected shell even when no substitutions exist", async () => {
  const { cwd, environment } = await shellContext();
  await expect(expandShell("plain", "/definitely/missing-shell", { rules: [], hasPermissions: false }, cwd, environment)).rejects.toThrow("shell executable");
  await expect(expandShell("plain", "/", { rules: [], hasPermissions: false }, cwd, environment)).rejects.toThrow("shell executable");
});
it("executes arithmetic expansion when permissions are unconfigured", async () => {
  const { cwd, environment } = await shellContext();
  await expect(expandShell("!`echo $((1 + 2))`", "/bin/sh", { rules: [], hasPermissions: false }, cwd, environment)).resolves.toBe("3\n");
});
it("discovers only complete exact expressions, preflights all commands, and does not recurse", async () => {
  const { cwd, environment } = await shellContext();
  expect(expressions("x !`printf one` !`printf two` !`open")).toHaveLength(2);
  await expect(expandShell("!`printf first` !`printf second`", "/bin/sh", { rules: [{ pattern: "printf first", action: "allow", source: "test" }], hasPermissions: true }, cwd, environment)).rejects.toThrow("second");
  await expect(expandShell("!`printf '\\041\\140nested\\140'`", "/bin/sh", { rules: [], hasPermissions: false }, cwd, environment)).resolves.toBe("!`nested`");
});
it("keeps stdout from failed commands, drains stderr, and runs in one concurrent batch", async () => {
  const { cwd, environment } = await shellContext(); const started = Date.now(); const output = await expandShell("!`sleep 0.2; printf a` !`sleep 0.2; printf b` !`printf kept; false` !`yes x | head -c 100000 >&2; false`", "/bin/sh", { rules: [], hasPermissions: false }, cwd, environment);
  expect(Date.now() - started).toBeLessThan(350); expect(output).toBe("a b kept ");
});
