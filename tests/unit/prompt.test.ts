import { mkdir, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { appendPrompt, parsePrompt, resolvePrompt } from "../../src/prompt.js";
import { removeTemp, tempDir } from "../helpers/temp.js";

const clean: string[] = [];
afterEach(async () => Promise.all(clean.splice(0).map(removeTemp)));

describe("prompt rendering", () => {
  it("removes canonical frontmatter and expands a skill home", () => {
    const parsed = parsePrompt("---\nqc_skill_path: ~/skills\nqc_tool: pi\n---\nhello", "p.md", "/home/test");
    expect(parsed).toMatchObject({
      body: "hello",
      meta: { skillPath: "/home/test/skills", tool: "pi" },
    });
  });

  it("formats the appended message exactly", () =>
    expect(appendPrompt("body", "more")).toBe("body\n\n---\n\nAdditional Message from the user:\n\nmore"));

  // rawAppend=true when no prompt file was loaded (append-only or continue+append-only)
  it("uses raw append text when no prompt file", () =>
    expect(appendPrompt("", "also add tests", true)).toBe("also add tests"));

  it("rejects removed qc_cli and qc_approve with rename errors", () => {
    expect(() => parsePrompt("---\nqc_cli: pi\n---\nbody", "p.md", "/home")).toThrow("qc_tool");
    expect(() => parsePrompt("---\nqc_approve: true\n---\nbody", "p.md", "/home")).toThrow("qc_approve");
  });

  it("rejects legacy frontmatter keys with qc guidance", () => {
    expect(() => parsePrompt("---\npqi_model: x\n---\nbody", "p.md", "/home")).toThrow("qc_*");
    expect(() => parsePrompt("---\nqpi_model: x\n---\nbody", "p.md", "/home")).toThrow("qc_*");
    expect(() => parsePrompt("---\npi_model: x\n---\nbody", "p.md", "/home")).toThrow("qc_*");
  });

  it.each(["---\n- item\n---\nbody", "---\nvalue\n---\nbody", "---\ntrue\n---\nbody"])(
    "requires a YAML mapping",
    (source) => {
      expect(() => parsePrompt(source, "p.md", "/home")).toThrow("mapping");
    },
  );

  it("validates description as a string", () =>
    expect(() => parsePrompt("---\ndescription: 4\n---\nbody", "p.md", "/home")).toThrow("description"));

  it("strips a folded comment key with the rest of the frontmatter", () => {
    const source = [
      "---",
      "description: documented",
      "comment: >",
      "  Example of driving an external CLI from a qc prompt.",
      "---",
      "body",
    ].join("\n");
    const parsed = parsePrompt(source, "p.md", "/home");
    expect(parsed.body).toBe("body");
    expect(parsed.meta).toEqual({});
  });

  it("validates every canonical qc field and leaves unknown non-qc metadata alone", () => {
    const source =
      "---\ndescription: documented\nowner: team\ncomment: human note\nqc_tool: cursor\nqc_model: m\nqc_thinking: high\nqc_workdir: /w\nqc_no_skills: true\nqc_skill_path: relative\n---\nbody";
    const parsed = parsePrompt(source, "p.md", "/home");
    expect(parsed).toMatchObject({
      body: "body",
      meta: {
        tool: "cursor",
        model: "m",
        thinking: "high",
        workdir: "/w",
        noSkills: true,
        skillPath: "relative",
      },
    });
    expect(parsed.meta).not.toHaveProperty("comment");
    expect(parsed.meta).not.toHaveProperty("owner");
    expect(parsed.body).not.toContain("human note");
    for (const field of [
      "qc_model: 1",
      "qc_thinking: ''",
      "qc_no_skills: yes",
      "qc_skill_path: ''",
      "qc_workdir: ''",
      "qc_unknown: x",
      "pi_model: x",
    ]) {
      expect(() => parsePrompt(`---\n${field}\n---\nbody`, "p.md", "/home")).toThrow();
    }
    expect(appendPrompt("body", undefined)).toBe("body");
    expect(appendPrompt("body", "")).toBe("body\n\n---\n\nAdditional Message from the user:\n\n");
  });

  it("classifies direct paths and resolves project/global aliases, nested aliases, and safe symlinks", async () => {
    const root = await tempDir("prompt-paths");
    clean.push(root);
    const cwd = join(root, "cwd");
    const home = join(root, "home");
    const project = join(cwd, ".qc", "prompts");
    const global = join(home, ".qc", "prompts");
    await mkdir(join(project, "nested"), { recursive: true });
    await mkdir(join(global, "nested"), { recursive: true });
    await writeFile(join(project, "same.md"), "project");
    await writeFile(join(global, "same.md"), "global");
    await writeFile(join(global, "nested", "global.md"), "nested");
    await writeFile(join(cwd, "direct.md"), "direct");
    await writeFile(join(project, "inside.md"), "inside");
    await symlink("inside.md", join(project, "link.md"));
    expect(await resolvePrompt("same", cwd, home)).toBe(join(project, "same.md"));
    expect(await resolvePrompt("nested/global", cwd, home)).toBe(join(global, "nested", "global.md"));
    expect(await resolvePrompt("direct.md", cwd, home)).toBe(join(cwd, "direct.md"));
    expect(await resolvePrompt("link", cwd, home)).toBe(join(project, "link.md"));
    await expect(resolvePrompt("nested/../escape", cwd, home)).rejects.toThrow("must stay");
    await expect(resolvePrompt("none", cwd, home)).rejects.toThrow(join(project, "none.md"));
  });

  it("rejects an alias symlink that escapes its selected prompt root", async () => {
    const root = await tempDir("prompt-escape");
    clean.push(root);
    const cwd = join(root, "cwd");
    const home = join(root, "home");
    const prompts = join(cwd, ".qc", "prompts");
    await mkdir(prompts, { recursive: true });
    const outside = join(root, "outside.md");
    await writeFile(outside, "outside");
    await symlink(outside, join(prompts, "escape.md"));
    await expect(resolvePrompt("escape", cwd, home)).rejects.toThrow("outside");
  });
});
