<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { baseName } from "../lib/paths";
  import { onMount, onDestroy } from "svelte";
  import {
    GitBranch,
    RefreshCw,
    ArrowDownToLine,
    ArrowUpToLine,
    GitPullRequestArrow,
    GitCommitHorizontal,
    FolderGit2,
    FolderTree,
    FileDiff,
    GitCommitVertical,
    GitBranchPlus,
    Search,
    TextSearch,
    Code2,
    Terminal,
    X,
  } from "@lucide/svelte";
  import { EditorView } from "@codemirror/view";
  import type { EditorState } from "@codemirror/state";
  import type { MergeView } from "@codemirror/merge";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { createFileState, createMergeView } from "../lib/editor";
  import type { Commit, DirEntry, GitStatus, Worktree } from "../lib/types";
  import FileTree from "./FileTree.svelte";

  let { paneId }: { paneId: string } = $props();

  const root = $derived(app.findPane(paneId)?.workingDirectory ?? app.home);

  type FTab = "files" | "changes" | "commits" | "worktrees";
  let gs = $state<GitStatus | null>(null);
  let rootEntries = $state<DirEntry[]>([]);
  let tab = $state<FTab>("files");
  let mode = $state<"edit" | "diff">("edit");
  let diffMode = $state<"split" | "unified">("split");
  let selected = $state<string | null>(null); // usado só no modo diff
  let diffText = $state("");
  let diffOld = $state("");
  let diffNew = $state("");
  let mergeHost = $state<HTMLElement>();
  let mergeView: MergeView | undefined;

  // Editor multi-abas: arquivos abertos + estado (undo/cursor) por arquivo.
  interface OpenFile {
    path: string;
    name: string;
    dirty: boolean;
  }
  let openFiles = $state<OpenFile[]>([]);
  let activeFilePath = $state<string | null>(null);
  const fileStates = new Map<string, EditorState>();
  let shownPath: string | null = null;
  let branchMenu = $state(false);
  let newBranch = $state("");
  let msg = $state("");

  let commits = $state<Commit[]>([]);
  let worktrees = $state<Worktree[]>([]);
  let commitMsg = $state("");
  let wtPath = $state("");
  let wtBranch = $state("");
  let wtCreate = $state(true);

  async function switchTab(t: FTab) {
    tab = t;
    if (t === "commits" && gs?.root) commits = await api.gitLog(gs.root, 60).catch(() => []);
    if (t === "worktrees" && gs?.root) worktrees = await api.gitWorktreeList(gs.root).catch(() => []);
  }

  async function showCommit(c: Commit) {
    if (!gs?.root) return;
    selected = c.short;
    mode = "diff";
    diffMode = "unified"; // um commit toca vários arquivos → diff unificado
    diffText = t("editor.loading");
    diffText = await api.gitShow(gs.root, c.hash).catch((e) => "erro: " + e);
  }

  async function doCommit() {
    const m = commitMsg.trim();
    if (!m || !gs?.root) return;
    try {
      await api.gitCommit(gs.root, m);
      commitMsg = "";
      flash(t("editor.commitDone"));
      await loadStatus();
      commits = await api.gitLog(gs.root, 60).catch(() => []);
    } catch (e) {
      flash(String(e));
    }
  }

  async function doPush() {
    if (!gs?.root) return;
    flash(t("git.push") + "…");
    try {
      await api.gitPush(gs.root);
      flash(t("git.pushOk"));
      await loadStatus();
    } catch (e) {
      flash(String(e));
    }
  }

  async function createWorktree() {
    const p = wtPath.trim();
    const b = wtBranch.trim();
    if (!p || !b || !gs?.root) return;
    try {
      await api.gitWorktreeAdd(gs.root, p, b, wtCreate);
      flash(t("editor.worktreeCreated"));
      wtPath = "";
      wtBranch = "";
      worktrees = await api.gitWorktreeList(gs.root).catch(() => []);
    } catch (e) {
      flash(String(e));
    }
  }

  let editorHost = $state<HTMLElement>();
  let editorView: EditorView | undefined;

  // Painel lateral (árvore/mudanças) redimensionável.
  let paneEl = $state<HTMLElement>();
  let resizingPanel = $state(false);
  function startPanelResize(e: PointerEvent) {
    e.preventDefault();
    resizingPanel = true;
    const left = paneEl?.getBoundingClientRect().left ?? 0;
    const move = (ev: PointerEvent) => app.setEditorPanelWidth(ev.clientX - left);
    const up = () => {
      resizingPanel = false;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  const statusMap = $derived.by(() => {
    const m: Record<string, string> = {};
    if (gs?.root) for (const f of gs.files) m[`${gs.root}/${f.path}`] = f.status;
    return m;
  });

  onMount(async () => {
    // Este editor passa a atender os pedidos de "abrir arquivo" da palette (⌘P/⌘⇧F).
    app.openInEditor = (path: string, line?: number) => void openFile(path, line);
    await Promise.all([loadStatus(), loadRoot()]);
  });
  onDestroy(() => {
    if (app.openInEditor) app.openInEditor = null;
    editorView?.destroy();
    mergeView?.destroy();
  });

  async function loadStatus() {
    try {
      gs = await api.gitStatus(root);
    } catch {
      gs = null;
    }
  }
  async function loadRoot() {
    try {
      rootEntries = await api.fsListDir(root);
    } catch {
      rootEntries = [];
    }
  }

  async function openFile(path: string, line?: number) {
    mode = "edit";
    if (!openFiles.some((f) => f.path === path)) {
      let content = "";
      try {
        content = await api.fsReadFile(path);
      } catch {
        content = "";
      }
      const name = baseName(path) || path;
      fileStates.set(
        path,
        createFileState(content, name, () => markDirty(path), (c) => saveFile(path, c)),
      );
      openFiles.push({ path, name, dirty: false });
    }
    activeFilePath = path;
    if (line) pendingLine = { path, line };
  }

  /** Linha a revelar assim que o arquivo virar o ativo (vindo da busca). */
  let pendingLine: { path: string; line: number } | null = null;

  function revealLine(view: EditorView, line: number) {
    const l = Math.max(1, Math.min(line, view.state.doc.lines));
    const pos = view.state.doc.line(l).from;
    view.dispatch({ selection: { anchor: pos }, scrollIntoView: true });
    view.focus();
  }

  function switchFile(path: string) {
    mode = "edit";
    activeFilePath = path;
  }

  function markDirty(path: string) {
    const f = openFiles.find((f) => f.path === path);
    if (f && !f.dirty) f.dirty = true;
  }

  function closeFile(path: string) {
    const idx = openFiles.findIndex((f) => f.path === path);
    if (idx < 0) return;
    openFiles.splice(idx, 1);
    fileStates.delete(path);
    if (shownPath === path) shownPath = null;
    if (activeFilePath === path) {
      activeFilePath = openFiles[Math.min(idx, openFiles.length - 1)]?.path ?? null;
    }
  }

  // Uma única EditorView; troca de estado ao mudar de aba (preserva undo/cursor).
  $effect(() => {
    if (mode !== "edit" || !editorHost) return;
    const path = activeFilePath;
    if (!path) return;
    const st = fileStates.get(path);
    if (!st) return;
    if (!editorView) {
      editorView = new EditorView({ state: st, parent: editorHost });
      shownPath = path;
      editorView.focus();
      if (pendingLine && pendingLine.path === path) {
        revealLine(editorView, pendingLine.line);
        pendingLine = null;
      }
      return;
    }
    if (path !== shownPath) {
      if (shownPath) fileStates.set(shownPath, editorView.state); // preserva edições
      editorView.setState(fileStates.get(path)!);
      shownPath = path;
      editorView.focus();
    }
    if (pendingLine && pendingLine.path === path && editorView) {
      revealLine(editorView, pendingLine.line);
      pendingLine = null;
    }
  });

  // (Re)cria o diff lado a lado quando as versões/host mudam.
  $effect(() => {
    if (mode !== "diff" || diffMode !== "split" || !mergeHost) return;
    const host = mergeHost;
    const o = diffOld;
    const n = diffNew;
    const f = selected ?? "";
    mergeView?.destroy();
    mergeView = createMergeView(host, o, n, f);
    return () => {
      mergeView?.destroy();
      mergeView = undefined;
    };
  });

  async function saveFile(path: string, content: string) {
    try {
      await api.fsWriteFile(path, content);
      const f = openFiles.find((f) => f.path === path);
      if (f) f.dirty = false;
      flash(t("editor.saved"));
      await loadStatus();
    } catch (e) {
      flash(t("editor.saveError", { error: String(e) }));
    }
  }

  function rel(path: string): string {
    return gs?.root && path.startsWith(gs.root + "/") ? path.slice(gs.root.length + 1) : path;
  }

  async function openDiff(relPath: string) {
    selected = gs?.root ? `${gs.root}/${relPath}` : relPath;
    mode = "diff";
    diffMode = "split";
    diffOld = "";
    diffNew = "";
    if (!gs?.root) return;
    try {
      const v = await api.gitFileVersions(gs.root, relPath);
      diffOld = v.old;
      diffNew = v.new;
    } catch (e) {
      diffMode = "unified";
      diffText = "erro: " + e;
    }
  }

  const diffLines = $derived(diffText.split("\n"));

  async function withRoot(fn: (r: string) => Promise<unknown>, ok: string) {
    if (!gs?.root) return;
    try {
      await fn(gs.root);
      flash(ok);
      await loadStatus();
    } catch (e) {
      flash(String(e));
    }
  }

  async function checkout(branch: string) {
    branchMenu = false;
    await withRoot((r) => api.gitCheckout(r, branch), t("git.switchedTo", { branch }));
    await loadRoot();
  }
  async function createBranch() {
    const name = newBranch.trim();
    if (!name) return;
    newBranch = "";
    branchMenu = false;
    await withRoot((r) => api.gitCreateBranch(r, name), t("git.branchCreated", { branch: name }));
  }

  let branches = $state<string[]>([]);
  async function toggleBranchMenu() {
    branchMenu = !branchMenu;
    if (branchMenu && gs?.root) {
      try {
        branches = await api.gitBranches(gs.root);
      } catch {
        branches = [];
      }
    }
  }

  let flashTimer: ReturnType<typeof setTimeout>;
  function flash(t: string) {
    msg = t;
    clearTimeout(flashTimer);
    // Erros (mais longos) ficam mais tempo; tudo é dispensável no clique.
    flashTimer = setTimeout(() => (msg = ""), t.length > 40 ? 8000 : 3000);
  }
</script>

<div class="files" bind:this={paneEl}>
  <!-- Barra git -->
  <div class="gitbar">
    {#if gs?.isRepo}
      <div class="branch-wrap">
        <button class="branch" onclick={toggleBranchMenu} title={"Branch: " + gs.branch}>
          <GitBranch size={14} /><span class="bname">{gs.branch}</span>{#if gs.dirty}<span class="dot"></span>{/if}
        </button>
        {#if gs.ahead > 0}<span class="ab">↑{gs.ahead}</span>{/if}
        {#if gs.behind > 0}<span class="ab">↓{gs.behind}</span>{/if}
        {#if branchMenu}
          <div class="menu">
            <div class="new">
              <input placeholder={t("git.newBranch")} bind:value={newBranch} onkeydown={(e) => e.key === "Enter" && createBranch()} />
              <button onclick={createBranch}>+</button>
            </div>
            {#each branches as b (b)}
              <div class="bitem" class:cur={b === gs.branch} onclick={() => checkout(b)} role="button" tabindex="0">{b}</div>
            {/each}
          </div>
        {/if}
      </div>
      <div class="spacer"></div>
      <button onclick={() => withRoot((r) => api.gitFetch(r), t("git.fetchOk"))} title={t("git.fetch")}><RefreshCw size={13} /> {t("git.fetch")}</button>
      <button onclick={() => withRoot((r) => api.gitPull(r), t("git.pullOk"))} title={t("git.pull")}><ArrowDownToLine size={13} /> {t("git.pull")}</button>
      <button onclick={() => gs?.root && api.gitOpenPr(gs.root).catch((e) => flash(String(e)))} title={t("git.pullRequests")}><GitPullRequestArrow size={13} /> PR</button>
    {:else}
      <span class="norepo">{t("git.noRepo", { path: root })}</span>
    {/if}
  </div>

  <!-- Abas só com ÍCONE (estilo Zed): largura fixa → o layout nunca "pula". -->
  <div class="tabs">
    <button class:active={tab === "files"} title={t("editor.files")} onclick={() => switchTab("files")}>
      <FolderTree size={16} />
    </button>
    <button class:active={tab === "changes"} title={t("editor.changes")} onclick={() => switchTab("changes")}>
      <FileDiff size={16} />
      {#if gs && gs.files.length}<span class="badge">{gs.files.length}</span>{/if}
    </button>
    {#if gs?.isRepo}
      <button class:active={tab === "commits"} title={t("editor.commits")} onclick={() => switchTab("commits")}>
        <GitCommitVertical size={16} />
      </button>
      <button class:active={tab === "worktrees"} title={t("editor.worktrees")} onclick={() => switchTab("worktrees")}>
        <GitBranchPlus size={16} />
      </button>
    {/if}
    <div class="tabs-spacer"></div>
    <button class="tool" title={t("search.quickOpen") + " (⌘P)"} onclick={() => app.openQuickOpen(root)}>
      <Search size={16} />
    </button>
    <button class="tool" title={t("search.globalSearch") + " (⌘⇧F)"} onclick={() => app.openGlobalSearch(root)}>
      <TextSearch size={16} />
    </button>
  </div>

  <div class="body" style="--pw:{app.settings.editorPanelWidth}px">
    <div class="sidepanel">
      {#if tab === "files"}
        {#each rootEntries as entry (entry.path)}
          <FileTree {entry} {statusMap} onOpen={openFile} {selected} />
        {/each}
      {:else if tab === "changes"}
        {#if gs?.isRepo}
          <div class="commitbox">
            <input placeholder={t("editor.commitPlaceholder")} bind:value={commitMsg} onkeydown={(e) => e.key === "Enter" && doCommit()} />
            <div class="cbtns">
              <button class="commit-btn" title="git add -A && git commit" disabled={!commitMsg.trim() || !gs.files.length} onclick={doCommit}>
                <GitCommitHorizontal size={14} /> {t("editor.commit")}{#if gs.files.length} ({gs.files.length}){/if}
              </button>
              <button class="push-btn" title="git push" onclick={doPush}>
                <ArrowUpToLine size={14} /> {t("git.push")}{#if gs.ahead} (↑{gs.ahead}){/if}
              </button>
            </div>
          </div>
        {/if}
        {#if gs && gs.files.length}
          {#each gs.files as f (f.path)}
            <div class="change" class:sel={selected === (gs.root ? gs.root + "/" + f.path : f.path)} onclick={() => openDiff(f.path)} role="button" tabindex="0">
              <span class="st">{f.status.trim() || "?"}</span>
              <span class="cpath">{f.path}</span>
            </div>
          {/each}
        {:else}
          <div class="empty">{t("editor.noChanges")}</div>
        {/if}
      {:else if tab === "commits"}
        {#each commits as c (c.hash)}
          <div class="commit" class:sel={selected === c.short} onclick={() => showCommit(c)} role="button" tabindex="0">
            <div class="csubj">{c.subject}</div>
            <div class="cmeta">{c.short} · {c.author} · {c.date}</div>
          </div>
        {:else}
          <div class="empty">{t("editor.noCommits")}</div>
        {/each}
      {:else if tab === "worktrees"}
        <div class="wtform">
          <input placeholder={t("editor.worktreePath")} bind:value={wtPath} />
          <input placeholder={t("editor.worktreeBranch")} bind:value={wtBranch} />
          <label class="wtchk"><input type="checkbox" bind:checked={wtCreate} /> {t("editor.worktreeNewBranch")}</label>
          <button disabled={!wtPath.trim() || !wtBranch.trim()} onclick={createWorktree}>
            <FolderGit2 size={14} /> {t("editor.createWorktree")}
          </button>
        </div>
        {#each worktrees as w (w.path)}
          <div class="wt" class:cur={w.path.replace(/\/$/, "") === root.replace(/\/$/, "")}>
            <div class="wtbranch">{w.branch || "(detached)"} <span class="wthead">{w.head}</span></div>
            <div class="wtpath">{w.path}</div>
            <div class="wtacts">
              <button title={t("editor.openEditorHere")} onclick={() => app.openFilesTab(w.path)}>
                <Code2 size={12} /> {t("pane.editor")}
              </button>
              <button title={t("editor.openSessionHere")} onclick={() => app.createTabInDir("claude", w.path, `⑂ ${w.branch || "wt"}`)}>
                <Terminal size={12} /> {t("editor.session")}
              </button>
            </div>
          </div>
        {:else}
          <div class="empty">{t("editor.noWorktrees")}</div>
        {/each}
      {/if}
    </div>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="pdivider" class:dragging={resizingPanel} onpointerdown={startPanelResize}></div>

    <div class="main">
      <!-- Abas dos arquivos abertos (estilo VSCodium) -->
      {#if mode === "edit" && openFiles.length}
        <div class="etabs">
          {#each openFiles as f (f.path)}
            <div
              class="etab"
              class:active={f.path === activeFilePath}
              onclick={() => switchFile(f.path)}
              role="button"
              tabindex="0"
              title={f.path}
            >
              <span class="en">{f.name}</span>
              {#if f.dirty}<span class="edot" title={t("editor.unsaved")}></span>{/if}
              <button class="ex" title={t("editor.closeFile")} onclick={(e) => { e.stopPropagation(); closeFile(f.path); }}><X size={12} /></button>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Editor (sempre montado; escondido no modo diff / sem arquivo) -->
      <div class="editor" bind:this={editorHost} style:display={mode === "edit" && openFiles.length ? "block" : "none"}></div>

      {#if mode === "edit" && openFiles.length === 0}
        <div class="empty center">{t("editor.selectFile")}</div>
      {:else if mode === "diff"}
        <div class="editor-head">
          {rel(selected ?? "")} — {t("editor.diff")}
          <span class="diff-legend">
            {#if diffMode === "split"}<span class="lg old">{t("editor.diffBefore")}</span><span class="lg new">{t("editor.diffAfter")}</span>{/if}
          </span>
        </div>
        {#if diffMode === "split"}
          <div class="merge" bind:this={mergeHost}></div>
        {:else}
          <div class="diff">
            {#each diffLines as line, i (i)}
              <div
                class="dl"
                class:add={line.startsWith("+") && !line.startsWith("+++")}
                class:del={line.startsWith("-") && !line.startsWith("---")}
                class:hunk={line.startsWith("@@")}
                class:meta={line.startsWith("diff ") || line.startsWith("index ") || line.startsWith("+++") || line.startsWith("---")}
              >{line || " "}</div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  </div>

  {#if msg}
    <button class="toast" onclick={() => (msg = "")} title={t("editor.dismiss")}>{msg}</button>
  {/if}
</div>

<style>
  .files {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #1e1e1e;
    color: #d4d4d4;
    overflow: hidden;
  }
  .gitbar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    flex: 0 0 30px;
    padding: 0 10px;
    background: #252526;
    border-bottom: 1px solid #2a2a2a;
    font-size: 12px;
    flex-wrap: nowrap;
    /* sem overflow:hidden (cortava o menu de branch); o branch já trunca via .bname */
  }
  .branch-wrap {
    position: relative;
    min-width: 0;
    max-width: 200px;
  }
  .branch {
    background: none;
    border: none;
    color: #cccccc;
    cursor: pointer;
    font-size: 12px;
    display: flex;
    align-items: center;
    gap: 5px;
    max-width: 200px;
    overflow: hidden;
  }
  .branch :global(svg),
  .branch .dot {
    flex: 0 0 auto;
  }
  .bname {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .toast {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 10px;
    z-index: 30;
    text-align: left;
    background: #2d2d30;
    border: 1px solid #3a3a3a;
    border-left: 3px solid #007acc;
    color: #d4d4d4;
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 12px;
    cursor: pointer;
    max-height: 30%;
    overflow-y: auto;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
    word-break: break-word;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #e2c08d;
    display: inline-block;
  }
  .ab {
    color: #9aa0a6;
    font-size: 11px;
  }
  .menu {
    position: absolute;
    top: 24px;
    left: 0;
    z-index: 20;
    background: #252526;
    border: 1px solid #3a3a3a;
    border-radius: 6px;
    padding: 4px;
    min-width: 200px;
    max-height: 300px;
    overflow-y: auto;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }
  .new {
    display: flex;
    gap: 4px;
    margin-bottom: 4px;
  }
  .new input {
    flex: 1 1 auto;
    background: #1e1e1e;
    border: 1px solid #3a3a3a;
    color: #fff;
    border-radius: 4px;
    padding: 3px 6px;
    min-width: 0;
  }
  .new button,
  .gitbar button:not(.branch) {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: #333;
    border: none;
    color: #ccc;
    border-radius: 4px;
    padding: 3px 8px;
    cursor: pointer;
    font-size: 11px;
  }
  .gitbar button:not(.branch):hover {
    background: #3f3f46;
    color: #fff;
  }
  .bitem {
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }
  .bitem:hover {
    background: #2a2d2e;
  }
  .bitem.cur {
    color: #4ec9b0;
  }
  .spacer {
    flex: 1 1 auto;
  }
  .norepo {
    color: #7a7a7a;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tabs {
    display: flex;
    height: 26px;
    background: #252526;
    border-bottom: 1px solid #2a2a2a;
  }
  .tabs button {
    position: relative;
    flex: 0 0 auto;
    width: 38px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: #8a8a8a;
    padding: 0;
    cursor: pointer;
    border-bottom: 2px solid transparent;
  }
  .tabs button:hover {
    color: #ddd;
    background: #2a2d2e;
  }
  .tabs .tool {
    width: 32px;
    color: #7a7a7a;
  }
  .tabs button.active {
    color: #fff;
    border-bottom-color: #007acc;
  }
  .badge {
    position: absolute;
    top: 3px;
    right: 3px;
    min-width: 13px;
    height: 13px;
    padding: 0 3px;
    box-sizing: border-box;
    background: #0e639c;
    color: #fff;
    border-radius: 7px;
    font-size: 9px;
    line-height: 13px;
    text-align: center;
  }
  .tabs-spacer {
    flex: 1 1 auto;
  }
  /* Cabeçalho do editor com altura fixa (não "pula" entre editar e diff). */
  .editor-head {
    flex: 0 0 22px;
  }
  /* Painéis das abas: mesma caixa, rolagem própria — sem salto de tamanho. */
  .sidepanel > :global(*) {
    max-width: 100%;
  }
  .body {
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: var(--pw, 240px) 1px minmax(0, 1fr);
    min-height: 0;
  }
  .sidepanel {
    overflow: hidden auto;
    /* Reserva o espaço da barra de rolagem SEMPRE: sem ela, abrir uma aba com
       lista longa (Commits) fazia a barra aparecer e cortar a largura. */
    scrollbar-gutter: stable;
    padding: 4px 0;
    min-width: 0;
  }
  /* Nada dentro do painel pode esticar a coluna (era isso que mudava o layout). */
  .sidepanel * {
    max-width: 100%;
    min-width: 0;
    box-sizing: border-box;
  }
  .main {
    min-width: 0;
    overflow: hidden;
  }
  .pdivider {
    position: relative;
    background: #2a2a2a;
    cursor: col-resize;
  }
  .pdivider::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: -3px;
    right: -3px;
  }
  .pdivider:hover,
  .pdivider.dragging {
    background: #007acc;
  }
  .change {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 3px 10px;
    font-size: 12.5px;
    cursor: pointer;
  }
  .change:hover {
    background: #2a2d2e;
  }
  .change.sel {
    background: #37373d;
  }
  .st {
    width: 16px;
    color: #e2c08d;
    font-family: monospace;
  }
  .cpath {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .commitbox {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border-bottom: 1px solid #2a2a2a;
    margin-bottom: 4px;
  }
  .commitbox input {
    background: #1e1e1e;
    border: 1px solid #3a3a3a;
    color: #fff;
    border-radius: 5px;
    padding: 5px 8px;
    outline: none;
    font-size: 12px;
  }
  .cbtns {
    display: flex;
    gap: 6px;
  }
  .cbtns button {
    flex: 1 1 0;
  }
  .commitbox button,
  .wtform button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    background: #0e639c;
    border: none;
    color: #fff;
    border-radius: 5px;
    padding: 6px;
    cursor: pointer;
    font-size: 12px;
  }
  .commitbox .push-btn {
    background: #2ea043;
  }
  .commitbox .push-btn:hover {
    background: #3fb950;
  }
  .commitbox button:disabled,
  .wtform button:disabled {
    background: #333;
    color: #777;
    cursor: default;
  }
  .commit {
    padding: 6px 10px;
    border-radius: 5px;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  .commit:hover {
    background: #2a2d2e;
  }
  .commit.sel {
    background: #37373d;
    border-left-color: #007acc;
  }
  .csubj {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cmeta {
    font-size: 10.5px;
    color: #8a8a8a;
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .commit {
    overflow: hidden;
  }
  .wtform {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border-bottom: 1px solid #2a2a2a;
    margin-bottom: 4px;
  }
  .wtform input[type="text"],
  .wtform input:not([type]) {
    background: #1e1e1e;
    border: 1px solid #3a3a3a;
    color: #fff;
    border-radius: 5px;
    padding: 5px 8px;
    outline: none;
    font-size: 12px;
  }
  .wtchk {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: #b8b8b8;
  }
  .wt {
    padding: 6px 10px;
    border-left: 2px solid transparent;
  }
  .wt.cur {
    border-left-color: #4ec9b0;
    background: #4ec9b012;
  }
  .wtacts {
    display: flex;
    gap: 6px;
    margin-top: 5px;
  }
  .wtacts button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: #333;
    border: none;
    color: #ccc;
    border-radius: 4px;
    padding: 3px 8px;
    cursor: pointer;
    font-size: 11px;
  }
  .wtacts button:hover {
    background: #3f3f46;
    color: #fff;
  }
  .wtbranch {
    font-size: 12.5px;
    color: #4ec9b0;
  }
  .wthead {
    color: #7a7a7a;
    font-family: monospace;
    font-size: 11px;
  }
  .wtpath {
    font-size: 11px;
    color: #8a8a8a;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .editor-head {
    height: 22px;
    line-height: 22px;
    padding: 0 10px;
    font-size: 11px;
    color: #9aa0a6;
    background: #252526;
    border-bottom: 1px solid #2a2a2a;
  }
  .etabs {
    display: flex;
    align-items: stretch;
    height: 30px;
    flex: 0 0 30px;
    background: #252526;
    border-bottom: 1px solid #2a2a2a;
    overflow-x: auto;
  }
  .etab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px 0 12px;
    max-width: 180px;
    font-size: 12px;
    color: #9aa0a6;
    cursor: pointer;
    border-right: 1px solid #2a2a2a;
    white-space: nowrap;
  }
  .etab:hover {
    background: #2a2d2e;
  }
  .etab.active {
    background: #1e1e1e;
    color: #fff;
  }
  .etab .en {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .edot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #e2c08d;
    flex: 0 0 auto;
  }
  .ex {
    display: flex;
    align-items: center;
    background: none;
    border: none;
    color: #6a6a6a;
    cursor: pointer;
    padding: 2px;
    border-radius: 3px;
  }
  .ex:hover {
    color: #eee;
    background: #4a4a4a;
  }
  .editor {
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
  }
  .merge {
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
  }
  .merge :global(.cm-mergeView),
  .merge :global(.cm-mergeViewEditors) {
    height: 100%;
  }
  .merge :global(.cm-editor) {
    height: 100%;
  }
  .diff-legend {
    float: right;
    display: inline-flex;
    gap: 10px;
  }
  .lg {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .lg::before {
    content: "";
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .lg.old::before {
    background: rgba(248, 81, 73, 0.5);
  }
  .lg.new::before {
    background: rgba(63, 185, 80, 0.5);
  }
  .diff {
    flex: 1 1 auto;
    overflow: auto;
    font-family: Menlo, monospace;
    font-size: 12px;
    padding: 6px 0;
  }
  .dl {
    padding: 0 10px;
    white-space: pre;
  }
  .dl.add {
    background: rgba(35, 134, 54, 0.18);
    color: #6ee7a0;
  }
  .dl.del {
    background: rgba(248, 81, 73, 0.15);
    color: #f8938d;
  }
  .dl.hunk {
    color: #3b8eea;
  }
  .dl.meta {
    color: #7a7a7a;
  }
  .empty {
    color: #7a7a7a;
    padding: 12px;
    font-size: 13px;
  }
  .center {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
