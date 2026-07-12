// Estado central do app (Svelte 5 runes) + ações sobre o manifest.
//
// Política de save (lição #7): mudança estrutural (criar/fechar/mover
// tab/pane/workspace/folder) salva SÍNCRONO; cwd e ratio de split usam debounce.

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { buildCommand, needsSessionId } from "./profiles";
import type {
  LayoutNode,
  Manifest,
  Pane,
  Settings,
  SplitDirection,
  Tab,
  Workspace,
} from "./types";

function uuid(): string {
  return crypto.randomUUID();
}
function newId(prefix: string): string {
  return `${prefix}_${uuid().replace(/-/g, "").slice(0, 12)}`;
}
function now(): number {
  return Date.now();
}

function leaf(paneId: string): LayoutNode {
  return { type: "leaf", paneId };
}

/** Constrói uma árvore de splits iguais a partir de uma lista de panes. */
function chain(paneIds: string[], direction: SplitDirection): LayoutNode {
  if (paneIds.length === 1) return leaf(paneIds[0]);
  const [first, ...rest] = paneIds;
  return {
    type: "split",
    id: newId("split"),
    direction,
    ratio: 1 / paneIds.length,
    children: [leaf(first), chain(rest, direction)],
  };
}

/** Grade: pares em linhas horizontais, empilhadas na vertical. */
function grid(paneIds: string[]): LayoutNode {
  const rows: LayoutNode[] = [];
  for (let i = 0; i < paneIds.length; i += 2) {
    const r = paneIds.slice(i, i + 2);
    rows.push(r.length === 1 ? leaf(r[0]) : chain(r, "horizontal"));
  }
  const stack = (nodes: LayoutNode[]): LayoutNode => {
    if (nodes.length === 1) return nodes[0];
    const [first, ...rest] = nodes;
    return {
      type: "split",
      id: newId("split"),
      direction: "vertical",
      ratio: 1 / nodes.length,
      children: [first, stack(rest)],
    };
  };
  return stack(rows);
}

function replaceLeaf(node: LayoutNode, paneId: string, withNode: LayoutNode): LayoutNode {
  if (node.type === "leaf") return node.paneId === paneId ? withNode : node;
  return {
    ...node,
    children: node.children.map((c) => replaceLeaf(c, paneId, withNode)),
  };
}

function removeLeaf(node: LayoutNode, paneId: string): LayoutNode | null {
  if (node.type === "leaf") return node.paneId === paneId ? null : node;
  const survivors = node.children
    .map((c) => removeLeaf(c, paneId))
    .filter((c): c is LayoutNode => c !== null);
  if (survivors.length === 0) return null;
  if (survivors.length === 1) return survivors[0];
  return { ...node, children: survivors };
}

function setRatio(node: LayoutNode, splitId: string, ratio: number): LayoutNode {
  if (node.type === "leaf") return node;
  return {
    ...node,
    ratio: node.id === splitId ? ratio : node.ratio,
    children: node.children.map((c) => setRatio(c, splitId, ratio)),
  };
}

function leavesOf(node: LayoutNode): string[] {
  return node.type === "leaf" ? [node.paneId] : node.children.flatMap(leavesOf);
}

export interface NameModal {
  kind: "newWorkspace" | "newFolder" | "rename";
  target?: { type: "ws" | "folder" | "tab"; id: string };
  title: string;
  name: string;
  directory: string | null;
  showDirectory: boolean;
}

export interface ConfirmState {
  title: string;
  message: string;
  confirmLabel: string;
  danger: boolean;
  onConfirm: () => void;
}

class AppStore {
  manifest = $state<Manifest>({ version: 3, activeWorkspaceId: null, workspaces: [] });
  settings = $state<Settings>({ yolo: false, fontSize: 13, webgl: false, shell: "" });
  loaded = $state(false);
  activePaneId = $state<string | null>(null);
  settingsOpen = $state(false);
  historyOpen = $state(false);
  usageOpen = $state(false);
  nameModal = $state<NameModal | null>(null);
  confirm = $state<ConfirmState | null>(null);
  home = "";

  /** Panes criados NESTA sessão (spawn fresco). Os demais, ao serem restaurados
   *  após o daemon morrer, usam comando de resume. */
  private freshPanes = new Set<string>();
  private saveTimer: ReturnType<typeof setTimeout> | undefined;

  async load(): Promise<void> {
    this.home = await api.homeDir();
    const [m, s] = await Promise.all([api.manifestLoad(), api.settingsLoad()]);
    this.manifest = m;
    this.settings = s;
    this.syncActivePane();
    this.loaded = true;
  }

  // ── Seleção ────────────────────────────────────────────────────────────
  get activeWorkspace(): Workspace | undefined {
    return (
      this.manifest.workspaces.find((w) => w.id === this.manifest.activeWorkspaceId) ??
      this.manifest.workspaces[0]
    );
  }

  get activeTab(): Tab | undefined {
    const w = this.activeWorkspace;
    if (!w) return undefined;
    return w.tabs.find((t) => t.id === w.activeTabId) ?? w.tabs[0];
  }

  findPane(paneId: string): Pane | undefined {
    for (const w of this.manifest.workspaces)
      for (const t of w.tabs) {
        const p = t.panes.find((p) => p.id === paneId);
        if (p) return p;
      }
    return undefined;
  }

  tabsInFolder(ws: Workspace, folderId: string | null): Tab[] {
    return ws.tabs.filter((t) => (t.folderId ?? null) === folderId);
  }

  // ── Save ──────────────────────────────────────────────────────────────
  save(): void {
    void api.manifestSave($state.snapshot(this.manifest));
  }
  saveDebounced(): void {
    clearTimeout(this.saveTimer);
    this.saveTimer = setTimeout(() => this.save(), 300);
  }
  saveSettings(): void {
    void api.settingsSave($state.snapshot(this.settings));
  }

  toggleYolo(): void {
    this.settings.yolo = !this.settings.yolo;
    this.saveSettings();
  }
  toggleWebgl(): void {
    this.settings.webgl = !this.settings.webgl;
    this.saveSettings();
  }
  setShell(path: string): void {
    this.settings.shell = path;
    this.saveSettings();
  }
  setFontSize(size: number): void {
    this.settings.fontSize = Math.max(8, Math.min(28, size));
    this.saveSettings();
  }

  // ── Workspaces ─────────────────────────────────────────────────────────
  selectWorkspace(id: string): void {
    this.manifest.activeWorkspaceId = id;
    this.syncActivePane();
    this.save();
  }

  /** Abre o seletor nativo de pasta. Devolve o caminho ou null (cancelado). */
  async pickDirectory(current?: string): Promise<string | null> {
    const res = await open({
      directory: true,
      multiple: false,
      defaultPath: current ?? this.home,
    });
    return typeof res === "string" ? res : null;
  }

  createWorkspaceNamed(name: string, dir: string): void {
    const ws: Workspace = {
      id: newId("ws"),
      name,
      order: this.manifest.workspaces.length,
      folders: [],
      tabs: [],
      activeTabId: null,
      directory: dir,
    };
    this.manifest.workspaces.push(ws);
    this.manifest.activeWorkspaceId = ws.id;
    this.createTab("shell");
    this.save();
  }

  // ── Modais de nomeação ─────────────────────────────────────────────────────
  openNewWorkspaceModal(): void {
    this.nameModal = {
      kind: "newWorkspace",
      title: "Novo workspace",
      name: "",
      directory: this.home,
      showDirectory: true,
    };
  }
  openNewFolderModal(): void {
    this.nameModal = {
      kind: "newFolder",
      title: "Nova pasta",
      name: "",
      directory: null,
      showDirectory: false,
    };
  }
  openRenameModal(type: "ws" | "folder" | "tab", id: string, current: string): void {
    const title = type === "ws" ? "Renomear workspace" : type === "folder" ? "Renomear pasta" : "Renomear aba";
    this.nameModal = { kind: "rename", target: { type, id }, title, name: current, directory: null, showDirectory: false };
  }
  async pickModalDirectory(): Promise<void> {
    const m = this.nameModal;
    if (!m) return;
    const dir = await this.pickDirectory(m.directory ?? this.home);
    if (dir) {
      m.directory = dir;
      if (!m.name.trim()) m.name = dir.split("/").filter(Boolean).pop() ?? "";
    }
  }
  confirmNameModal(): void {
    const m = this.nameModal;
    if (!m) return;
    const name = m.name.trim();
    if (m.kind === "newWorkspace") {
      if (name && m.directory) this.createWorkspaceNamed(name, m.directory);
    } else if (m.kind === "newFolder") {
      this.createFolder(name || "Nova pasta");
    } else if (m.kind === "rename" && m.target) {
      if (name) {
        if (m.target.type === "ws") this.renameWorkspace(m.target.id, name);
        else if (m.target.type === "folder") this.renameFolder(m.target.id, name);
        else this.renameTab(m.target.id, name);
      }
    }
    this.nameModal = null;
  }

  // ── Confirmação antes de excluir/fechar ────────────────────────────────────
  askConfirm(opts: {
    title: string;
    message: string;
    confirmLabel?: string;
    danger?: boolean;
    onConfirm: () => void;
  }): void {
    this.confirm = {
      title: opts.title,
      message: opts.message,
      confirmLabel: opts.confirmLabel ?? "Excluir",
      danger: opts.danger ?? true,
      onConfirm: opts.onConfirm,
    };
  }
  runConfirm(): void {
    const c = this.confirm;
    this.confirm = null;
    c?.onConfirm();
  }
  confirmDeleteWorkspace(id: string): void {
    const ws = this.manifest.workspaces.find((w) => w.id === id);
    this.askConfirm({
      title: "Excluir workspace",
      message: `Excluir "${ws?.name ?? ""}"? Todas as abas e sessões dele serão encerradas.`,
      onConfirm: () => this.deleteWorkspace(id),
    });
  }
  confirmDeleteFolder(id: string): void {
    const f = this.activeWorkspace?.folders.find((f) => f.id === id);
    this.askConfirm({
      title: "Excluir pasta",
      message: `Excluir a pasta "${f?.name ?? ""}"? As abas dentro dela voltam para a raiz (não são apagadas).`,
      onConfirm: () => this.deleteFolder(id),
    });
  }
  confirmCloseTab(id: string): void {
    const tab = this.activeWorkspace?.tabs.find((t) => t.id === id);
    this.askConfirm({
      title: "Fechar aba",
      message: `Fechar "${tab?.title ?? ""}"? A sessão será encerrada.`,
      confirmLabel: "Fechar",
      onConfirm: () => this.closeTab(id),
    });
  }
  confirmClosePane(id: string): void {
    this.askConfirm({
      title: "Fechar painel",
      message: "Fechar este painel? A sessão será encerrada.",
      confirmLabel: "Fechar",
      onConfirm: () => this.closePane(id),
    });
  }

  async changeWorkspaceDirectory(id: string): Promise<void> {
    const ws = this.manifest.workspaces.find((w) => w.id === id);
    const dir = await this.pickDirectory(ws?.directory ?? this.home);
    if (dir) this.setWorkspaceDirectory(id, dir);
  }

  renameWorkspace(id: string, name: string): void {
    const ws = this.manifest.workspaces.find((w) => w.id === id);
    if (ws) {
      ws.name = name;
      this.save();
    }
  }

  deleteWorkspace(id: string): void {
    const ws = this.manifest.workspaces.find((w) => w.id === id);
    if (ws) ws.tabs.forEach((t) => t.panes.forEach((p) => this.killPane(p.id)));
    this.manifest.workspaces = this.manifest.workspaces.filter((w) => w.id !== id);
    if (this.manifest.activeWorkspaceId === id) {
      this.manifest.activeWorkspaceId = this.manifest.workspaces[0]?.id ?? null;
    }
    this.syncActivePane();
    this.save();
  }

  setWorkspaceDirectory(id: string, dir: string): void {
    const ws = this.manifest.workspaces.find((w) => w.id === id);
    if (ws) {
      ws.directory = dir;
      this.save();
    }
  }

  // ── Folders ────────────────────────────────────────────────────────────
  /** Cria uma pasta e devolve o id (a UI abre o rename inline para nomear). */
  createFolder(name = "Nova pasta"): string {
    const ws = this.activeWorkspace;
    if (!ws) return "";
    const id = newId("fold");
    ws.folders.push({ id, name, order: ws.folders.length, collapsed: false });
    this.save();
    return id;
  }

  async changeFolderDirectory(id: string): Promise<void> {
    const f = this.activeWorkspace?.folders.find((f) => f.id === id);
    const dir = await this.pickDirectory(f?.directory ?? this.activeWorkspace?.directory ?? this.home);
    if (dir) this.setFolderDirectory(id, dir);
  }
  renameFolder(id: string, name: string): void {
    const f = this.activeWorkspace?.folders.find((f) => f.id === id);
    if (f) {
      f.name = name;
      this.save();
    }
  }
  toggleFolder(id: string): void {
    const f = this.activeWorkspace?.folders.find((f) => f.id === id);
    if (f) {
      f.collapsed = !f.collapsed;
      this.save();
    }
  }
  deleteFolder(id: string): void {
    const ws = this.activeWorkspace;
    if (!ws) return;
    // As abas da pasta voltam pra raiz (não são apagadas).
    ws.tabs.forEach((t) => {
      if (t.folderId === id) t.folderId = null;
    });
    ws.folders = ws.folders.filter((f) => f.id !== id);
    this.save();
  }
  setFolderDirectory(id: string, dir: string): void {
    const f = this.activeWorkspace?.folders.find((f) => f.id === id);
    if (f) {
      f.directory = dir;
      this.save();
    }
  }

  // ── Tabs ──────────────────────────────────────────────────────────────
  private cwdFor(ws: Workspace, folderId: string | null): string {
    const folder = folderId ? ws.folders.find((f) => f.id === folderId) : undefined;
    return folder?.directory ?? ws.directory ?? this.home ?? ".";
  }

  private makePane(profileId: string, cwd: string): Pane {
    const pane: Pane = {
      id: newId("pane"),
      kind: "terminal",
      toolProfileId: profileId,
      workingDirectory: cwd,
      harnessSessionId: needsSessionId(profileId) ? uuid() : null,
      resumeExisting: false,
      scrollbackFile: null,
      createdAt: now(),
      updatedAt: now(),
    };
    this.freshPanes.add(pane.id); // criado agora → spawn fresco (não resume)
    return pane;
  }

  createTab(profileId: string, folderId: string | null = null): void {
    const ws = this.activeWorkspace;
    if (!ws) return;
    const pane = this.makePane(profileId, this.cwdFor(ws, folderId));
    const tab: Tab = {
      id: newId("tab"),
      folderId,
      title: profileId,
      panes: [pane],
      layout: leaf(pane.id),
      activePaneId: pane.id,
      createdAt: now(),
      updatedAt: now(),
    };
    ws.tabs.push(tab);
    ws.activeTabId = tab.id;
    this.activePaneId = pane.id;
    this.save();
  }

  private makeFilesPane(cwd: string): Pane {
    // Pane de arquivos não abre PTY; não entra em freshPanes.
    return {
      id: newId("pane"),
      kind: "files",
      toolProfileId: "shell",
      workingDirectory: cwd,
      harnessSessionId: null,
      resumeExisting: false,
      scrollbackFile: null,
      createdAt: now(),
      updatedAt: now(),
    };
  }

  /** Abre um visualizador de arquivos numa aba nova (raiz = dir do workspace). */
  openFilesTab(): void {
    const ws = this.activeWorkspace;
    if (!ws) return;
    const pane = this.makeFilesPane(ws.directory ?? this.home);
    const tab: Tab = {
      id: newId("tab"),
      folderId: null,
      title: "arquivos",
      panes: [pane],
      layout: leaf(pane.id),
      activePaneId: pane.id,
      createdAt: now(),
      updatedAt: now(),
    };
    ws.tabs.push(tab);
    ws.activeTabId = tab.id;
    this.activePaneId = pane.id;
    this.save();
  }

  /** Divide o pane ativo colocando um visualizador de arquivos ao lado. */
  splitFilesBeside(paneId: string): void {
    const ws = this.activeWorkspace;
    const tab = this.activeTab;
    if (!ws || !tab) return;
    const src = tab.panes.find((p) => p.id === paneId);
    const pane = this.makeFilesPane(src?.workingDirectory ?? ws.directory ?? this.home);
    tab.panes.push(pane);
    tab.layout = replaceLeaf(tab.layout, paneId, {
      type: "split",
      id: newId("split"),
      direction: "horizontal",
      ratio: 0.5,
      children: [leaf(paneId), leaf(pane.id)],
    });
    tab.activePaneId = pane.id;
    this.activePaneId = pane.id;
    this.save();
  }

  selectTab(id: string): void {
    const ws = this.activeWorkspace;
    if (!ws) return;
    ws.activeTabId = id;
    this.syncActivePane();
    this.save();
  }

  renameTab(id: string, title: string): void {
    const tab = this.activeWorkspace?.tabs.find((t) => t.id === id);
    if (tab) {
      tab.title = title;
      this.save();
    }
  }

  closeTab(id: string): void {
    const ws = this.activeWorkspace;
    if (!ws) return;
    const tab = ws.tabs.find((t) => t.id === id);
    if (tab) tab.panes.forEach((p) => this.killPane(p.id));
    ws.tabs = ws.tabs.filter((t) => t.id !== id);
    if (ws.activeTabId === id) ws.activeTabId = ws.tabs[0]?.id ?? null;
    this.syncActivePane();
    this.save();
  }

  moveTab(tabId: string, folderId: string | null, beforeTabId?: string): void {
    const ws = this.activeWorkspace;
    if (!ws) return;
    const idx = ws.tabs.findIndex((t) => t.id === tabId);
    if (idx < 0) return;
    const [tab] = ws.tabs.splice(idx, 1);
    tab.folderId = folderId;
    if (beforeTabId && beforeTabId !== tabId) {
      const bidx = ws.tabs.findIndex((t) => t.id === beforeTabId);
      ws.tabs.splice(bidx < 0 ? ws.tabs.length : bidx, 0, tab);
    } else {
      ws.tabs.push(tab);
    }
    this.save();
  }

  // ── Panes / splits ───────────────────────────────────────────────────────
  splitPane(paneId: string, direction: SplitDirection, profileId = "shell"): void {
    const ws = this.activeWorkspace;
    const tab = this.activeTab;
    if (!ws || !tab) return;
    const pane = this.makePane(profileId, this.cwdFor(ws, tab.folderId ?? null));
    tab.panes.push(pane);
    tab.layout = replaceLeaf(tab.layout, paneId, {
      type: "split",
      id: newId("split"),
      direction,
      ratio: 0.5,
      children: [leaf(paneId), leaf(pane.id)],
    });
    tab.activePaneId = pane.id;
    this.activePaneId = pane.id;
    this.save();
  }

  closePane(paneId: string): void {
    const tab = this.activeTab;
    if (!tab) return;
    this.killPane(paneId);
    const newLayout = removeLeaf(tab.layout, paneId);
    tab.panes = tab.panes.filter((p) => p.id !== paneId);
    if (!newLayout) {
      this.closeTab(tab.id);
      return;
    }
    tab.layout = newLayout;
    if (tab.activePaneId === paneId) {
      tab.activePaneId = leavesOf(tab.layout)[0] ?? null;
      this.activePaneId = tab.activePaneId;
    }
    this.save();
  }

  setActivePane(paneId: string): void {
    const tab = this.activeTab;
    if (tab) tab.activePaneId = paneId;
    this.activePaneId = paneId;
    this.saveDebounced();
  }

  setSplitRatio(splitId: string, ratio: number): void {
    const tab = this.activeTab;
    if (!tab) return;
    tab.layout = setRatio(tab.layout, splitId, Math.max(0.1, Math.min(0.9, ratio)));
    this.saveDebounced();
  }

  /** Presets de layout: rearranja os panes existentes da aba ativa. */
  arrange(mode: "columns" | "rows" | "grid"): void {
    const tab = this.activeTab;
    if (!tab || tab.panes.length === 0) return;
    const ids = tab.panes.map((p) => p.id);
    tab.layout =
      mode === "grid" ? grid(ids) : chain(ids, mode === "columns" ? "horizontal" : "vertical");
    this.save();
  }

  // ── Internos ─────────────────────────────────────────────────────────────
  private killPane(paneId: string): void {
    void invoke("terminal_kill", { paneId });
  }

  private syncActivePane(): void {
    const tab = this.activeTab;
    this.activePaneId = tab ? tab.activePaneId ?? leavesOf(tab.layout)[0] ?? null : null;
  }

  commandFor(pane: Pane): string | null {
    // Pane criado nesta sessão → fresco; carregado do manifest → resume (a aba
    // volta e a CLI retoma a MESMA conversa após reboot).
    return buildCommand(pane, this.settings, this.freshPanes.has(pane.id));
  }

  /** Abre uma sessão do histórico numa aba nova, retomando-a pelo id. */
  openHistorySession(rec: {
    harness: string;
    sessionId: string;
    projectPath: string;
    title?: string | null;
  }): void {
    const ws = this.activeWorkspace;
    if (!ws) return;
    const pane: Pane = {
      id: newId("pane"),
      kind: "terminal",
      toolProfileId: rec.harness,
      workingDirectory: rec.projectPath,
      harnessSessionId: rec.sessionId,
      resumeExisting: true, // → comando de resume específico por id
      scrollbackFile: null,
      createdAt: now(),
      updatedAt: now(),
    };
    // NÃO entra em freshPanes: queremos o comando de resume.
    const tab: Tab = {
      id: newId("tab"),
      folderId: null,
      title: rec.title?.slice(0, 24) || `${rec.harness} ↩`,
      panes: [pane],
      layout: leaf(pane.id),
      activePaneId: pane.id,
      createdAt: now(),
      updatedAt: now(),
    };
    ws.tabs.push(tab);
    ws.activeTabId = tab.id;
    this.activePaneId = pane.id;
    this.historyOpen = false;
    this.save();
  }
}

export const app = new AppStore();
