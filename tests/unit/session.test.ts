import { mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { realpathSync } from "node:fs";
import { afterEach, expect, it } from "vitest";
import {
  cwdKey,
  findLatestSession,
  loadSession,
  mintSessionId,
  parseSessionId,
  saveSession,
  sessionPath,
} from "../../src/session.js";
import { removeTemp, tempDir } from "../helpers/temp.js";

const clean: string[] = [];
afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));

it("mints local-time pretty ids with tool and 6-char suffix", () => {
  const now = new Date(2026, 8, 1, 17, 57, 0); // Sep is month 8
  const id = mintSessionId("pi", now);
  expect(id).toMatch(/^260901-1757--pi--[a-z0-9]{6}$/);
  expect(parseSessionId(id)).toMatchObject({ tool: "pi", stamp: "260901-1757" });
});

it("rejects invalid session ids", () => {
  expect(() => parseSessionId("bad")).toThrow("invalid session id");
  expect(() => parseSessionId("260901-1757--nope--abcdef")).toThrow("invalid session id tool");
});

it("saves and loads session mappings under ~/.qc/sessions", async () => {
  const root = await tempDir("session");
  clean.push(root);
  const home = join(root, "home");
  const id = "260901-1757--cursor--a1b2c3";
  await saveSession(home, id, {
    tool: "cursor",
    native_id: "uuid-1",
    cwd: "/work",
    created: "2026-09-01T00:00:00.000Z",
    updated: "2026-09-01T00:01:00.000Z",
    warnings: ["qc_thinking ignored"],
  });
  expect(JSON.parse(await readFile(sessionPath(home, id), "utf8"))).toMatchObject({
    tool: "cursor",
    native_id: "uuid-1",
    cwd: "/work",
    warnings: ["qc_thinking ignored"],
  });
  expect(await loadSession(home, id)).toMatchObject({ tool: "cursor", native_id: "uuid-1" });
  await expect(loadSession(home, "260901-1757--pi--zzzzzz")).rejects.toThrow("session not found");
});

it("cwdKey resolves then realpaths existing paths and does not walk ancestors", async () => {
  const root = await tempDir("cwd-key");
  clean.push(root);
  const real = join(root, "real");
  const link = join(root, "link");
  await mkdir(real);
  await symlink(real, link);
  expect(cwdKey(link)).toBe(realpathSync(real));
  expect(cwdKey(real)).toBe(cwdKey(link));
  const missing = join(root, "missing", "dir");
  expect(cwdKey(missing)).toBe(resolve(missing));
  expect(cwdKey(join(real, "child"))).not.toBe(cwdKey(real));
});

it("findLatestSession picks newest updated then created then id for the invocation cwd", async () => {
  const root = await tempDir("latest-session");
  clean.push(root);
  const home = join(root, "home");
  const cwd = join(root, "work");
  await mkdir(cwd);
  const older = "260901-1000--pi--aaaaaa";
  const newer = "260901-1000--pi--bbbbbb";
  const otherDir = "260901-1000--pi--cccccc";
  await saveSession(home, older, {
    tool: "pi",
    native_id: "old-native",
    cwd,
    created: "2026-09-01T02:00:00.000Z",
    updated: "2026-09-01T03:00:00.000Z",
    warnings: [],
  });
  await saveSession(home, newer, {
    tool: "pi",
    native_id: "new-native",
    cwd,
    created: "2026-09-01T01:00:00.000Z",
    updated: "2026-09-01T04:00:00.000Z",
    warnings: [],
  });
  await saveSession(home, otherDir, {
    tool: "pi",
    native_id: "elsewhere",
    cwd: join(root, "other"),
    created: "2026-09-01T09:00:00.000Z",
    updated: "2026-09-01T09:00:00.000Z",
    warnings: [],
  });
  const found = await findLatestSession(home, cwd);
  expect(found).toMatchObject({ id: newer, mapping: { native_id: "new-native" } });
});

it("findLatestSession skips corrupt files on scan and filters by tool", async () => {
  const root = await tempDir("latest-skip");
  clean.push(root);
  const home = join(root, "home");
  const cwd = join(root, "work");
  await mkdir(cwd);
  const piId = "260901-1200--pi--aaaaaa";
  const cursorId = "260901-1200--cursor--bbbbbb";
  await saveSession(home, piId, {
    tool: "pi",
    native_id: "pi-native",
    cwd,
    created: "2026-09-01T00:00:00.000Z",
    updated: "2026-09-01T00:00:00.000Z",
    warnings: [],
  });
  await saveSession(home, cursorId, {
    tool: "cursor",
    native_id: "cursor-native",
    cwd,
    created: "2026-09-01T01:00:00.000Z",
    updated: "2026-09-01T01:00:00.000Z",
    warnings: [],
  });
  await mkdir(join(home, ".qc", "sessions"), { recursive: true });
  await writeFile(join(home, ".qc", "sessions", "260901-1200--pi--zzzzzz.json"), "{not json", "utf8");
  await writeFile(join(home, ".qc", "sessions", "not-a-session.json"), '{"tool":"pi"}\n', "utf8");
  expect(await findLatestSession(home, cwd)).toMatchObject({ id: cursorId });
  expect(await findLatestSession(home, cwd, "pi")).toMatchObject({ id: piId });
  await expect(findLatestSession(home, cwd, "claude")).rejects.toThrow("no claude session found for this directory");
  await expect(findLatestSession(home, join(root, "empty"))).rejects.toThrow("no session found for this directory");
});
