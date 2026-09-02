import { readFile, realpath, stat } from "node:fs/promises";
import { isAbsolute, join, relative, resolve } from "node:path";
import { isMap, parseDocument } from "yaml";
import { QcError } from "./errors.js";
import { unsupportedCli } from "./messages.js";

export interface PromptMeta {
  model?: string;
  thinking?: string;
  noSkills?: boolean;
  skillPath?: string;
  approve?: boolean;
  cli?: string;
}

export interface Prompt {
  path: string;
  body: string;
  meta: PromptMeta;
}

const V1_CLI = "pi";

const isDirect = (reference: string) =>
  isAbsolute(reference) || reference.startsWith("./") || reference.startsWith("../") || reference.endsWith(".md");

async function regularFile(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isFile();
  } catch {
    return false;
  }
}

async function contained(candidate: string, root: string): Promise<boolean> {
  const [file, base] = await Promise.all([realpath(candidate), realpath(root)]);
  const diff = relative(base, file);
  return diff !== "" && !diff.startsWith("..") && !isAbsolute(diff);
}

export async function resolvePrompt(reference: string, cwd: string, home: string): Promise<string> {
  if (isDirect(reference)) {
    const direct = resolve(cwd, reference);
    if (!(await regularFile(direct))) throw new QcError(`prompt file not found: ${direct}`);
    return direct;
  }
  if (reference.split("/").includes("..") || reference.startsWith("/")) {
    throw new QcError(`prompt alias must stay below its prompts root: ${reference}`);
  }
  const name = `${reference}.md`;
  const roots = [join(cwd, ".qc", "prompts"), join(home, ".qc", "prompts")];
  const candidates = roots.map((root) => join(root, name));
  for (let index = 0; index < candidates.length; index += 1) {
    if (!(await regularFile(candidates[index]!))) continue;
    if (!(await contained(candidates[index]!, roots[index]!))) {
      throw new QcError(`prompt alias resolves outside its prompts root: ${candidates[index]!}`);
    }
    return candidates[index]!;
  }
  throw new QcError(
    `prompt alias '${reference}' was not found. Checked:\n${candidates.map((path) => `  ${path}`).join("\n")}`,
  );
}

export function parsePrompt(source: string, path: string, home: string): Prompt {
  let body = source;
  const meta: PromptMeta = {};
  if (source.startsWith("---\n") || source.startsWith("---\r\n")) {
    const match = source.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
    if (!match) throw new QcError(`invalid YAML frontmatter in ${path}`);
    const doc = parseDocument(match[1]!);
    if (doc.errors.length || !isMap(doc.contents)) throw new QcError(`YAML frontmatter in ${path} must be a mapping`);
    const values = doc.toJS() as Record<string, unknown>;
    const known = new Set([
      "description",
      "qc_model",
      "qc_thinking",
      "qc_no_skills",
      "qc_skill_path",
      "qc_approve",
      "qc_cli",
    ]);
    for (const key of Object.keys(values)) {
      if (key.startsWith("pqi_") || key.startsWith("pi_") || key.startsWith("qpi_")) {
        throw new QcError(`frontmatter key '${key}' in ${path} is not supported; use qc_* keys`);
      }
      if (key.startsWith("qc_") && !known.has(key)) throw new QcError(`unknown qc frontmatter key '${key}' in ${path}`);
    }
    const string = (key: "qc_model" | "qc_thinking" | "qc_skill_path" | "qc_cli"): string | undefined => {
      const value = values[key];
      if (value === undefined) return undefined;
      if (typeof value !== "string" || !value) throw new QcError(`${key} in ${path} must be a non-empty string`);
      return value;
    };
    const bool = (key: "qc_no_skills" | "qc_approve"): boolean | undefined => {
      const value = values[key];
      if (value === undefined) return undefined;
      if (typeof value !== "boolean") throw new QcError(`${key} in ${path} must be true or false`);
      return value;
    };
    if (values.description !== undefined && typeof values.description !== "string") {
      throw new QcError(`description in ${path} must be a string`);
    }
    meta.model = string("qc_model");
    meta.thinking = string("qc_thinking");
    meta.noSkills = bool("qc_no_skills");
    const skill = string("qc_skill_path");
    meta.skillPath = skill?.startsWith("~/") ? join(home, skill.slice(2)) : skill;
    meta.approve = bool("qc_approve");
    const cli = string("qc_cli");
    if (cli !== undefined && cli !== V1_CLI) throw new QcError(`qc_cli in ${path}: ${unsupportedCli(cli)}`);
    meta.cli = cli;
    body = source.slice(match[0].length);
  }
  return { path, body, meta };
}

export async function readPrompt(reference: string, cwd: string, home: string): Promise<Prompt> {
  const path = await resolvePrompt(reference, cwd, home);
  return parsePrompt(await readFile(path, "utf8"), path, home);
}

export const appendPrompt = (body: string, append: string | undefined): string =>
  append === undefined ? body : `${body}\n\n---\n\nAdditional Message from the user:\n\n${append}`;
