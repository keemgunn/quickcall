export type { ConfigPaths } from "./paths.js";
export { configPaths } from "./paths.js";
export {
  bootstrapGlobal,
  bootstrapSettings,
  hardRefreshSamplePrompts,
  installSamplePrompts,
  REFERENCE_GITIGNORE_ENTRIES,
  type BootstrapMode,
} from "./bootstrap.js";
export type { Config, ToolConfig, ToolDefaults } from "./parse.js";
export { loadConfig, effectiveShell } from "./merge.js";
