// Wrappers tipados sobre os comandos Tauri.

import { invoke } from "@tauri-apps/api/core";
import type {
  AcpImage,
  AcpMention,
  Commit,
  DirEntry,
  GitStatus,
  HarnessId,
  HarnessInfo,
  Manifest,
  McpServer,
  SessionRecord,
  SearchHit,
  Settings,
  ShellOption,
  Skill,
  Theme,
  UsageStats,
  Worktree,
} from "./types";

export const api = {
  manifestLoad: () => invoke<Manifest>("manifest_load"),
  manifestSave: (manifest: Manifest) => invoke<void>("manifest_save", { manifest }),
  settingsLoad: () => invoke<Settings>("settings_load"),
  settingsSave: (settings: Settings) => invoke<void>("settings_save", { settings }),
  homeDir: () => invoke<string>("home_dir"),
  listShells: () => invoke<ShellOption[]>("list_shells"),
  savePasteImage: (dataB64: string) => invoke<string>("save_paste_image", { dataB64 }),
  sessionHistoryLoad: () => invoke<SessionRecord[]>("session_history_load"),
  sessionTranscript: (record: SessionRecord) =>
    invoke<string>("session_transcript", { record }),
  usageLoad: () => invoke<UsageStats[]>("usage_load"),

  // Filesystem + git (M5)
  fsListDir: (path: string) => invoke<DirEntry[]>("fs_list_dir", { path }),
  fsReadFile: (path: string) => invoke<string>("fs_read_file", { path }),
  fsWriteFile: (path: string, content: string) =>
    invoke<void>("fs_write_file", { path, content }),
  fsListFiles: (root: string, limit?: number) =>
    invoke<string[]>("fs_list_files", { root, limit }),
  searchInFiles: (root: string, query: string, caseSensitive = false, limit?: number) =>
    invoke<SearchHit[]>("search_in_files", { root, query, caseSensitive, limit }),
  replaceInFiles: (root: string, query: string, replacement: string, files: string[]) =>
    invoke<number>("replace_in_files", { root, query, replacement, files }),
  gitStatus: (path: string) => invoke<GitStatus>("git_status", { path }),
  gitDiff: (root: string, file: string) => invoke<string>("git_diff", { root, file }),
  gitFileVersions: (root: string, file: string) =>
    invoke<{ old: string; new: string }>("git_file_versions", { root, file }),
  gitBranches: (root: string) => invoke<string[]>("git_branches", { root }),
  gitCheckout: (root: string, branch: string) =>
    invoke<void>("git_checkout", { root, branch }),
  gitCreateBranch: (root: string, branch: string) =>
    invoke<void>("git_create_branch", { root, branch }),
  gitFetch: (root: string) => invoke<void>("git_fetch", { root }),
  gitPull: (root: string) => invoke<string>("git_pull", { root }),
  gitPush: (root: string) => invoke<string>("git_push", { root }),
  gitOpenPr: (root: string) => invoke<void>("git_open_pr", { root }),
  gitLog: (root: string, limit = 50) => invoke<Commit[]>("git_log", { root, limit }),
  gitShow: (root: string, hash: string) => invoke<string>("git_show", { root, hash }),
  gitCommit: (root: string, message: string) => invoke<string>("git_commit", { root, message }),
  // Temas
  themesList: () => invoke<Theme[]>("themes_list"),
  themeActive: (id: string) => invoke<Theme>("theme_active", { id }),
  themeImportZed: (path: string) => invoke<Theme[]>("theme_import_zed", { path }),
  themeRemove: (id: string) => invoke<void>("theme_remove", { id }),

  // MCP e skills dos harnesses
  harnessList: () => invoke<HarnessInfo[]>("harness_list"),
  mcpUpsert: (harness: HarnessId, server: McpServer) =>
    invoke<void>("mcp_upsert", { harness, server }),
  mcpRemove: (harness: HarnessId, name: string) =>
    invoke<void>("mcp_remove", { harness, name }),
  mcpSetEnabled: (harness: HarnessId, name: string, enabled: boolean) =>
    invoke<void>("mcp_set_enabled", { harness, name, enabled }),
  mcpCopyTo: (from: HarnessId, to: HarnessId, name: string) =>
    invoke<void>("mcp_copy_to", { from, to, name }),
  skillsList: (harness: HarnessId, project?: string) =>
    invoke<Skill[]>("skills_list", { harness, project }),
  skillInstall: (harness: HarnessId, source: string, project?: string) =>
    invoke<Skill>("skill_install", { harness, source, project }),
  skillRemove: (harness: HarnessId, path: string, project?: string) =>
    invoke<void>("skill_remove", { harness, path, project }),

  gitWorktreeList: (root: string) => invoke<Worktree[]>("git_worktree_list", { root }),
  gitWorktreeAdd: (root: string, path: string, branch: string, create: boolean) =>
    invoke<string>("git_worktree_add", { root, path, branch, create }),
  createProjectWorktree: (repo: string, base: string, name: string) =>
    invoke<string>("create_project_worktree", { repo, base, name }),

  // Modo ACP — a sessão vive no daemon; `acpSpawn` também atacha (traz o replay).
  acpSpawn: (
    paneId: string,
    cwd: string,
    program: string,
    args: string[],
    allowTerminal: boolean,
  ) => invoke<void>("acp_spawn", { paneId, cwd, program, args, allowTerminal }),
  acpPrompt: (
    paneId: string,
    text: string,
    images: AcpImage[] = [],
    mentions: AcpMention[] = [],
  ) => invoke<void>("acp_prompt", { paneId, text, images, mentions }),
  acpCancel: (paneId: string) => invoke<void>("acp_cancel", { paneId }),
  acpFork: (
    paneId: string,
    sourceSessionId: string,
    cwd: string,
    program: string,
    args: string[],
    allowTerminal: boolean,
  ) =>
    invoke<void>("acp_fork", {
      paneId,
      sourceSessionId,
      cwd,
      program,
      args,
      allowTerminal,
    }),
  acpSetMode: (paneId: string, modeId: string) =>
    invoke<void>("acp_set_mode", { paneId, modeId }),
  acpSetModel: (paneId: string, modelId: string) =>
    invoke<void>("acp_set_model", { paneId, modelId }),
  acpPermission: (paneId: string, requestId: number, optionId: string | null) =>
    invoke<void>("acp_permission", { paneId, requestId, optionId }),
};
