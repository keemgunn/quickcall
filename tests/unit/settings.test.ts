import { expect, it } from "vitest";
import { resolveSettings } from "../../src/settings.js";
import type { Config } from "../../src/config/parse.js";

const base: Config = {
  tool: {
    default: "pi",
    tools: {
      pi: { defaultModel: "pi/global", defaultThinking: "medium" },
      cursor: { defaultModel: "composer-2.5" },
      claude: { defaultModel: "sonnet", defaultThinking: "high" },
    },
  },
};

it("applies flag > frontmatter > config precedence", () => {
  const resolved = resolveSettings(
    { tool: "claude", model: "opus" },
    { tool: "cursor", model: "ignored", thinking: "low" },
    base,
  );
  expect(resolved).toMatchObject({ tool: "claude", model: "opus", thinking: "low", command: "claude" });
});

it("warns and ignores inapplicable fields", () => {
  const cursor = resolveSettings({ thinking: "high", noSkills: true, skillPath: "/s" }, { tool: "cursor" }, base);
  expect(cursor.thinking).toBeUndefined();
  expect(cursor.noSkills).toBeUndefined();
  expect(cursor.skillPath).toBeUndefined();
  expect(cursor.warnings.some((w) => w.includes("qc_thinking"))).toBe(true);
  expect(cursor.warnings.some((w) => w.includes("qc_no_skills"))).toBe(true);

  const claude = resolveSettings({ thinking: "off", noSkills: true }, { tool: "claude" }, base);
  expect(claude.thinking).toBeUndefined();
  expect(claude.warnings.some((w) => w.includes("unsupported for claude"))).toBe(true);

  const agy = resolveSettings({ thinking: "xhigh" }, { tool: "antigravity" }, base);
  expect(agy.thinking).toBeUndefined();
  expect(agy.warnings.some((w) => w.includes("antigravity"))).toBe(true);
});

it("locks tool from session mapping and errors on mismatch", () => {
  expect(() => resolveSettings({ tool: "claude" }, {}, base, "pi")).toThrow("tool mismatch");
  expect(() => resolveSettings({}, { tool: "claude" }, base, "pi")).toThrow("tool mismatch");
  // Config default alone does not mismatch a continued session tool.
  expect(resolveSettings({}, {}, base, "cursor")).toMatchObject({ tool: "cursor", command: "agent" });
});

it("defaults tool to pi and uses per-tool command overrides", () => {
  const config: Config = {
    ...base,
    tool: { default: "pi", tools: { pi: { command: "custom-pi" } } },
  };
  expect(resolveSettings({}, {}, config).command).toBe("custom-pi");
  expect(resolveSettings({ tool: "cursor" }, {}, base).command).toBe("agent");
  expect(resolveSettings({ tool: "antigravity" }, {}, base).command).toBe("agy");
});
