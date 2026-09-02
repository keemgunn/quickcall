export type { ConfigPaths } from "./paths.js";
export { configPaths } from "./paths.js";
export {
  bootstrapGlobal,
  bootstrapSettings,
  hardRefreshSamplePrompts,
  installSamplePrompts,
  REFERENCE_GITIGNORE_ENTRY,
  type BootstrapMode,
} from "./bootstrap.js";
export type { Config, PermissionRule } from "./parse.js";
export { loadConfig, effectiveShell, resolveCli } from "./merge.js";
