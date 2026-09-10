import { access, readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const repoRoot = resolve(packageRoot, "..");
const runner = join(repoRoot, "scripts", "development", "test-sample.sh");
const sampleProject = join(repoRoot, "data", "dev", "sample-project");
const runRoot = join(repoRoot, ".tmp", "sample-run");

function runSample() {
  const child = spawn(runner, [], { cwd: packageRoot, env: process.env });
  let stdout = "";
  let stderr = "";
  const result = new Promise<{ code: number | null; stdout: string; stderr: string }>((resolveResult, reject) => {
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code) => resolveResult({ code, stdout, stderr }));
  });

  return result;
}

it("runs the visible sample through the built CLI in isolated fixture state", async () => {
  const result = await runSample();
  const recordPath = join(runRoot, "fixture-pi-record.json");
  const configPath = join(runRoot, "home", ".qc", "config.toml");

  expect(result.code).toBe(0);
  expect(result.stdout).toContain("fixture Pi: success");
  await expect(access(recordPath)).resolves.toBeUndefined();
  const record = JSON.parse(await readFile(recordPath, "utf8"));
  expect(record.argv).toEqual([
    "--mode",
    "json",
    "-a",
    "--session-id",
    expect.stringMatching(/^\d{6}-\d{4}--pi--[a-z0-9]{6}$/),
    "--model",
    "opencode-go/muse-spark-1.3-contributor",
    "--thinking",
    "high",
    "--no-skills",
  ]);
  expect(record.stdin).toEqual(expect.stringContaining(sampleProject));
  expect(await readFile(configPath)).toEqual(await readFile(join(packageRoot, "share/settings", "config.toml")));
});
