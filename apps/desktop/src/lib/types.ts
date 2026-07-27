// Espelho TS do manifest v3 (perene-core::models). Wire em camelCase.

export type Id = string;
export type PaneKind = "terminal" | "files";
export type SplitDirection = "horizontal" | "vertical";

export type LayoutNode =
  | { type: "leaf"; paneId: Id }
  | {
      type: "split";
      id: Id;
      direction: SplitDirection;
      ratio: number;
      children: LayoutNode[];
    };

export interface Pane {
  id: Id;
  kind: PaneKind;
  toolProfileId: string;
  workingDirectory: string;
  harnessSessionId?: string | null;
  resumeExisting: boolean;
  scrollbackFile?: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface Tab {
  id: Id;
  folderId?: string | null;
  title: string;
  panes: Pane[];
  layout: LayoutNode;
  activePaneId?: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface Folder {
  id: Id;
  name: string;
  order: number;
  collapsed: boolean;
  directory?: string | null;
}

export interface Workspace {
  id: Id;
  name: string;
  order: number;
  folders: Folder[];
  tabs: Tab[];
  activeTabId?: string | null;
  directory?: string | null;
}

export interface Manifest {
  version: number;
  activeWorkspaceId?: string | null;
  workspaces: Workspace[];
}

export interface Settings {
  yolo: boolean;
  fontSize: number;
  webgl: boolean;
  shell: string; // "" = padrão do sistema
  askWorktree: boolean;
  sidebarWidth: number;
  editorPanelWidth: number;
  locale: string; // "" = seguir o sistema
}

export interface ShellOption {
  path: string;
  label: string;
}

export interface SessionRecord {
  harness: string; // "claude" | "codex" | "opencode"
  sessionId: string;
  projectPath: string;
  title?: string | null;
  dateMs: number;
  sourcePath?: string | null;
}

export interface UsageStats {
  harness: string;
  sessions: number;
  input: number;
  output: number;
  cost: number;
}

export interface DirEntry {
  name: string;
  path: string;
  isDir: boolean;
}

export interface GitFile {
  path: string;
  status: string; // código de 2 chars do porcelain
}

export interface GitStatus {
  isRepo: boolean;
  root?: string | null;
  branch: string;
  ahead: number;
  behind: number;
  dirty: boolean;
  files: GitFile[];
}

export interface Commit {
  hash: string;
  short: string;
  subject: string;
  author: string;
  date: string;
}

export interface Worktree {
  path: string;
  branch: string;
  head: string;
}
