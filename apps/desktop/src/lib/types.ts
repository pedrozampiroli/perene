// Espelho TS do manifest v3 (perene-core::models). Wire em camelCase.

export type Id = string;
export type PaneKind = "terminal" | "files" | "acp";
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
  /** Presente = o pane bifurca esta sessão. Vazio = a mais recente do diretório. */
  forkFromSessionId?: string | null;
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
  onboardingDone: boolean;
  theme: string; // "" = tema embutido `dark-plus`
  /** Abrir sessões novas como chat ACP (só as ferramentas com adapter). */
  acpMode: boolean;
  /** No modo ACP, deixar o agente pedir que o Perene rode comandos. */
  acpTerminal: boolean;
}

// -- Temas -------------------------------------------------------------------
// Espelham `perene_core::theme`. Todo token da `ui` vira uma CSS var (--bg, …).

export interface ThemeUiColors {
  bg: string;
  fg: string;
  panel: string;
  elevated: string;
  border: string;
  muted: string;
  accent: string;
  accentFg: string;
  danger: string;
  warning: string;
  success: string;
  selection: string;
}

export interface ThemeTerminalColors {
  background: string;
  foreground: string;
  cursor: string;
  selectionBackground: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
}

export interface ThemeSyntaxColors {
  comment: string;
  keyword: string;
  string: string;
  number: string;
  function: string;
  typeName: string;
  variable: string;
  constant: string;
  operator: string;
  punctuation: string;
  property: string;
  tag: string;
}

export interface Theme {
  id: string;
  name: string;
  light: boolean;
  ui: ThemeUiColors;
  terminal: ThemeTerminalColors;
  syntax: ThemeSyntaxColors;
}

// -- Harnesses (MCP / skills) ------------------------------------------------

export type HarnessId = "claude" | "codex" | "opencode";

export interface McpServer {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  url: string;
  enabled: boolean;
}

export interface HarnessInfo {
  id: HarnessId;
  configPath: string;
  configured: boolean;
  servers: McpServer[];
  error: string | null;
}

export interface Skill {
  name: string;
  description: string;
  path: string;
  projectScoped: boolean;
  /** Veio de `.agents/skills`: mexer ali afeta codex E opencode. */
  shared: boolean;
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

/** O que a sessão está fazendo (indicador discreto na UI). */
export type PaneState = "idle" | "running" | "waiting" | "done" | "error";

export interface SearchHit {
  path: string;
  line: number;
  text: string;
}

export interface Worktree {
  path: string;
  branch: string;
  head: string;
}

// ── Modo ACP ────────────────────────────────────────────────────────────────
// Espelho de `perene_protocol::AcpEvent`. O `update` vem cru do agente (o
// protocolo evolui mais rápido que a nossa UI), por isso é `unknown`.

export interface AcpPermissionOption {
  optionId: string;
  name: string;
  kind?: string | null;
}

export type AcpEvent =
  | { kind: "ready"; sessionId: string; modes: AcpModes | null; models: AcpModels | null }
  | {
      kind: "terminal";
      terminalId: string;
      output: string;
      truncated: boolean;
      exitCode: number | null;
    }
  | { kind: "update"; update: Record<string, unknown> }
  | {
      kind: "permission";
      requestId: number;
      toolCall: Record<string, unknown>;
      options: AcpPermissionOption[];
    }
  | { kind: "turnEnded"; stopReason: string }
  | { kind: "failed"; message: string };

export interface AcpMessage {
  paneId: string;
  event: AcpEvent;
}

/** Um modo de permissão (Default, Accept Edits, Plan…) ou modelo. */
export interface AcpOption {
  id: string;
  name: string;
  description?: string | null;
}

export interface AcpModes {
  currentModeId: string;
  availableModes: AcpOption[];
}

/** Modelos usam `modelId` em vez de `id` — normalizamos ao ler. */
export interface AcpModels {
  currentModelId: string;
  availableModels: { modelId: string; name: string; description?: string | null }[];
}

/** Comando de barra anunciado pela sessão (`/context`, `/init`, skills…). */
export interface AcpCommand {
  name: string;
  description: string;
  /** `{ hint }` quando o comando aceita argumento. */
  input?: { hint?: string | null } | null;
}

/** Arquivo mencionado com `@` — vai como link, não como conteúdo. */
export interface AcpMention {
  path: string;
  name: string;
}

/** Imagem colada, indo junto do prompt. */
export interface AcpImage {
  dataB64: string;
  mimeType: string;
}
