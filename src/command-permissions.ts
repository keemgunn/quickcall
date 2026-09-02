import { parse, type Word } from "unbash";
import { QcError } from "./errors.js";
import type { PermissionRule } from "./config/index.js";
export const wildcard = (pattern: string): RegExp => new RegExp(`^${[...pattern].map((character) => character === "*" ? ".*" : character === "?" ? "." : character.replace(/[|\\{}()[\]^$+.]/g, "\\$&")).join("")}$`);
const dynamic = (word: Word) => /[$`]/.test(word.text) || word.text.includes("$(");
const inspectionError = (source: string): QcError => new QcError(`could not safely inspect shell expression: ${source}`);
function hasUnclosedCommandSubstitution(source: string): boolean {
  let quote = ""; let escaped = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index]!;
    if (escaped) { escaped = false; continue; }
    if (character === "\\") { escaped = true; continue; }
    if (quote) { if (character === quote) { quote = ""; continue; } if (quote === "'") continue; }
    if (character === "'" || character === '"') { quote = character; continue; }
    if (character !== "$" || source[index + 1] !== "(" || source[index + 2] === "(") continue;
    let depth = 1; let innerQuote = ""; let innerEscaped = false; index += 2;
    for (; index < source.length; index += 1) {
      const inner = source[index]!;
      if (innerEscaped) { innerEscaped = false; continue; }
      if (inner === "\\") { innerEscaped = true; continue; }
      if (innerQuote) { if (inner === innerQuote) innerQuote = ""; continue; }
      if (inner === "'" || inner === '"') { innerQuote = inner; continue; }
      if (inner === "(") depth += 1;
      if (inner === ")" && --depth === 0) break;
    }
    if (depth !== 0) return true;
  }
  return false;
}
function hasUnclosedArithmeticExpansion(source: string): boolean {
  let quote = ""; let escaped = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index]!;
    if (escaped) { escaped = false; continue; }
    if (character === "\\") { escaped = true; continue; }
    if (quote) { if (character === quote) { quote = ""; continue; } if (quote === "'") continue; }
    if (character === "'" || character === '"') { quote = character; continue; }
    if (character !== "$" || source[index + 1] !== "(" || source[index + 2] !== "(") continue;
    let depth = 2; let innerQuote = ""; let innerEscaped = false; index += 3;
    for (; index < source.length; index += 1) {
      const inner = source[index]!;
      if (innerEscaped) { innerEscaped = false; continue; }
      if (inner === "\\") { innerEscaped = true; continue; }
      if (innerQuote) { if (inner === innerQuote) innerQuote = ""; continue; }
      if (inner === "'" || inner === '"') { innerQuote = inner; continue; }
      if (inner === "(") depth += 1;
      if (inner === ")" && --depth === 0) break;
    }
    if (depth !== 0) return true;
  }
  return false;
}
export function commandSegments(source: string): string[] {
  if (hasUnclosedCommandSubstitution(source) || hasUnclosedArithmeticExpansion(source)) throw inspectionError(source);
  const tree = parse(source); if (tree.errors?.length) throw inspectionError(source);
  const result: string[] = []; const seen = new Set<object>();
  const visit = (value: unknown): void => {
    if (!value || typeof value !== "object" || seen.has(value as object)) return; seen.add(value as object);
    const node = value as { type?: string; name?: Word; pos?: number; end?: number; parts?: unknown[]; expression?: unknown; [key: string]: unknown };
    if (node.type === "Command") { if (!node.name || dynamic(node.name)) throw new QcError(`could not safely inspect dynamic shell command: ${source}`); result.push(source.slice(node.pos!, node.end!).trim()); }
    if (node.type === "ArithmeticExpansion") throw inspectionError(source);
    for (const child of Object.values(node)) { if (Array.isArray(child)) child.forEach(visit); else visit(child); }
    for (const part of node.parts ?? []) visit(part);
  };
  visit(tree);
  if (!result.length) throw inspectionError(source); return result;
}
export function authorize(source: string, rules: PermissionRule[], configured: boolean): void {
  if (!configured) return;
  for (const segment of commandSegments(source)) {
    let matched: PermissionRule | undefined; for (const rule of rules) if (wildcard(rule.pattern).test(segment)) matched = rule;
    if (!matched) throw new QcError(`shell command denied (no matching permission rule): ${segment}`);
    if (matched.action === "deny") throw new QcError(`shell command denied by '${matched.pattern}' in ${matched.source}: ${segment}`);
  }
}
