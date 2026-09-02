import { dirname, join } from "node:path";

export interface ConfigPaths {
  global: string;
  project: string;
  starter: string;
  starterPrompts: string;
  globalRoot: string;
  defaultSettings: string;
  gitignore: string;
  samplePrompts: string;
  starterSamplePrompts: string;
  bundledSettings: string;
}

/** Resolve every global and starter path qc bootstrap reads or writes. */
export const configPaths = (home: string, cwd: string, starter: string): ConfigPaths => {
  const globalRoot = join(home, ".qc");
  const bundledSettings = dirname(starter);
  return {
    global: join(globalRoot, "config.toml"),
    project: join(cwd, ".qc", "config.toml"),
    starter,
    starterPrompts: join(bundledSettings, "prompts"),
    globalRoot,
    defaultSettings: join(globalRoot, ".default-settings"),
    gitignore: join(globalRoot, ".gitignore"),
    samplePrompts: join(globalRoot, "prompts", "samples"),
    starterSamplePrompts: join(bundledSettings, "prompts", "samples"),
    bundledSettings,
  };
};
