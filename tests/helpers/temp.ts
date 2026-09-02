import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { randomUUID } from "node:crypto";
import { fileURLToPath } from "node:url";
const scratch = resolve(dirname(fileURLToPath(import.meta.url)), "../../../.tmp");
export async function tempDir(label: string): Promise<string> { await mkdir(scratch, { recursive: true }); return mkdtemp(join(scratch, `${label}-${randomUUID()}-`)); }
export async function removeTemp(path: string): Promise<void> { await rm(path, { recursive: true, force: true }); }
