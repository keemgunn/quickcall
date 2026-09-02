import { describe, expect, it } from "vitest";
import { authorize, commandSegments, wildcard } from "../../src/command-permissions.js";
const rules = [{ pattern: "*", action: "allow" as const, source: "test" }, { pattern: "rm -fr *", action: "deny" as const, source: "test" }];
describe("command permissions", () => {
  it("extracts every compound and command-substitution executable segment", () => expect(commandSegments("echo $(printf start && (printf x; rm -fr cache) | cat)")).toEqual(["echo $(printf start && (printf x; rm -fr cache) | cat)", "printf start", "printf x", "rm -fr cache", "cat"]));
  it("anchors wildcard rules and applies the last match", () => { expect(wildcard("git ?").test("git s")).toBe(true); expect(wildcard("git ?").test("git status")).toBe(false); expect(() => authorize("echo start && rm -fr cache", rules, true)).toThrow("denied"); });
  it("is case-sensitive, supports zero-or-many wildcards, and fails closed only when configured", () => {
    expect(wildcard("echo*").test("echo")).toBe(true); expect(wildcard("Echo *").test("echo x")).toBe(false); expect(() => authorize("whoami", [{ pattern: "echo *", action: "allow", source: "test" }], true)).toThrow("no matching"); expect(() => authorize("whoami", [], false)).not.toThrow();
  });
  it.each([
    ["printf a | cat", ["printf a", "cat"]],
    ["printf a; printf b", ["printf a", "printf b"]],
    ["if printf test; then printf yes; else printf no; fi", ["printf test", "printf yes", "printf no"]],
    ["(printf one; printf two)", ["printf one", "printf two"]],
    ["echo $(printf nested)", ["echo $(printf nested)", "printf nested"]],
    ["cat <(printf input)", ["cat <(printf input)", "printf input"]],
    ["cat >(printf output)", ["cat >(printf output)", "printf output"]],
    ["cat <<EOF\n$(printf heredoc)\nEOF", ["cat <<EOF", "printf heredoc"]],
  ])("extracts exact segments from %s", (command, expected) => expect(commandSegments(command)).toEqual(expected));
  it("fails configured permission inspection closed for malformed and dynamic command names", () => {
    expect(() => authorize("echo $(printf", rules, true)).toThrow("could not safely inspect shell expression");
    expect(() => authorize("echo $((1 + 2)", rules, true)).toThrow("could not safely inspect shell expression");
    expect(() => authorize("$command value", rules, true)).toThrow("could not safely inspect dynamic shell command");
  });
  it("fails configured arithmetic command substitution inspection without returning a fake command", () => {
    expect(() => authorize("echo $((1 + $(printf 2)))", rules, true)).toThrow("could not safely inspect shell expression");
  });
  it("fails closed for all configured arithmetic expansions but bypasses unconfigured inspection", () => {
    expect(() => authorize("echo $((1 + 2))", [{ pattern: "echo $((1 + 2))", action: "allow", source: "test" }], true)).toThrow("could not safely inspect shell expression");
    expect(() => authorize("echo $((1 + 2))", [], false)).not.toThrow();
  });
});
