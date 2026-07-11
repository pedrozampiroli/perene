<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import type { EditorView } from "@codemirror/view";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { createEditor } from "../lib/editor";
  import type { DirEntry, GitStatus } from "../lib/types";
  import FileTree from "./FileTree.svelte";

  let { paneId }: { paneId: string } = $props();

  const root = $derived(app.findPane(paneId)?.workingDirectory ?? app.home);

  let gs = $state<GitStatus | null>(null);
  let rootEntries = $state<DirEntry[]>([]);
  let tab = $state<"files" | "changes">("files");
  let mode = $state<"edit" | "diff">("edit");
  let selected = $state<string | null>(null);
  let currentContent = $state("");
  let diffText = $state("");
  let dirtyDoc = $state(false);
  let branchMenu = $state(false);
  let newBranch = $state("");
  let msg = $state("");

  let editorHost = $state<HTMLElement>();
  let editorView: EditorView | undefined;

  const statusMap = $derived.by(() => {
    const m: Record<string, string> = {};
    if (gs?.root) for (const f of gs.files) m[`${gs.root}/${f.path}`] = f.status;
    return m;
  });

  onMount(async () => {
    await Promise.all([loadStatus(), loadRoot()]);
  });
  onDestroy(() => editorView?.destroy());

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

  async function openFile(path: string) {
    selected = path;
    mode = "edit";
    try {
      currentContent = await api.fsReadFile(path);
    } catch {
      currentContent = "";
    }
    dirtyDoc = false;
  }

  // (Re)cria o editor quando o arquivo/host muda.
  $effect(() => {
    if (mode !== "edit" || !editorHost || selected === null) return;
    const host = editorHost;
    const file = selected;
    const content = currentContent;
    editorView?.destroy();
    editorView = createEditor(host, content, file, saveFile);
    editorView.dom.addEventListener("input", () => (dirtyDoc = true));
    return () => {
      editorView?.destroy();
      editorView = undefined;
    };
  });

  async function saveFile(content: string) {
    if (!selected) return;
    try {
      await api.fsWriteFile(selected, content);
      dirtyDoc = false;
      flash("Salvo");
      await loadStatus();
    } catch (e) {
      flash("Erro ao salvar: " + e);
    }
  }

  function rel(path: string): string {
    return gs?.root && path.startsWith(gs.root + "/") ? path.slice(gs.root.length + 1) : path;
  }

  async function openDiff(relPath: string) {
    selected = gs?.root ? `${gs.root}/${relPath}` : relPath;
    mode = "diff";
    diffText = "";
    if (!gs?.root) return;
    try {
      diffText = (await api.gitDiff(gs.root, relPath)) || "(sem diferenças)";
    } catch (e) {
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
    await withRoot((r) => api.gitCheckout(r, branch), `→ ${branch}`);
    await loadRoot();
  }
  async function createBranch() {
    const name = newBranch.trim();
    if (!name) return;
    newBranch = "";
    branchMenu = false;
    await withRoot((r) => api.gitCreateBranch(r, name), `criado ${name}`);
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
    flashTimer = setTimeout(() => (msg = ""), 2500);
  }
</script>

<div class="files">
  <!-- Barra git -->
  <div class="gitbar">
    {#if gs?.isRepo}
      <div class="branch-wrap">
        <button class="branch" onclick={toggleBranchMenu} title="Trocar/criar branch">
          ⑂ {gs.branch}{#if dirtyDoc || gs.dirty}<span class="dot"></span>{/if}
        </button>
        {#if gs.ahead > 0}<span class="ab">↑{gs.ahead}</span>{/if}
        {#if gs.behind > 0}<span class="ab">↓{gs.behind}</span>{/if}
        {#if branchMenu}
          <div class="menu">
            <div class="new">
              <input placeholder="novo branch…" bind:value={newBranch} onkeydown={(e) => e.key === "Enter" && createBranch()} />
              <button onclick={createBranch}>+</button>
            </div>
            {#each branches as b (b)}
              <div class="bitem" class:cur={b === gs.branch} onclick={() => checkout(b)} role="button" tabindex="0">{b}</div>
            {/each}
          </div>
        {/if}
      </div>
      <div class="spacer"></div>
      <button onclick={() => withRoot((r) => api.gitFetch(r), "fetch ok")} title="Fetch">⟳ fetch</button>
      <button onclick={() => withRoot((r) => api.gitPull(r), "pull ok")} title="Pull">↓ pull</button>
      <button onclick={() => gs?.root && api.gitOpenPr(gs.root).catch((e) => flash(String(e)))} title="Abrir PR no browser">PR ↗</button>
    {:else}
      <span class="norepo">Sem repositório git em {root}</span>
    {/if}
    {#if msg}<span class="msg">{msg}</span>{/if}
  </div>

  <div class="tabs">
    <button class:active={tab === "files"} onclick={() => (tab = "files")}>Files</button>
    <button class:active={tab === "changes"} onclick={() => (tab = "changes")}>
      Changes{#if gs && gs.files.length}<span class="count">{gs.files.length}</span>{/if}
    </button>
  </div>

  <div class="body">
    <div class="sidepanel">
      {#if tab === "files"}
        {#each rootEntries as entry (entry.path)}
          <FileTree {entry} {statusMap} onOpen={openFile} {selected} />
        {/each}
      {:else if gs && gs.files.length}
        {#each gs.files as f (f.path)}
          <div class="change" class:sel={selected === (gs.root ? gs.root + "/" + f.path : f.path)} onclick={() => openDiff(f.path)} role="button" tabindex="0">
            <span class="st">{f.status.trim() || "?"}</span>
            <span class="cpath">{f.path}</span>
          </div>
        {/each}
      {:else}
        <div class="empty">Nenhuma mudança.</div>
      {/if}
    </div>

    <div class="main">
      {#if selected === null}
        <div class="empty center">Selecione um arquivo.</div>
      {:else if mode === "edit"}
        <div class="editor-head">{rel(selected)}{#if dirtyDoc} •{/if}</div>
        <div class="editor" bind:this={editorHost}></div>
      {:else}
        <div class="editor-head">{rel(selected)} — diff</div>
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
    </div>
  </div>
</div>

<style>
  .files {
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
    padding: 0 10px;
    background: #252526;
    border-bottom: 1px solid #2a2a2a;
    font-size: 12px;
  }
  .branch-wrap {
    position: relative;
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
  }
  .msg {
    color: #4ec9b0;
    font-size: 11px;
  }
  .tabs {
    display: flex;
    height: 26px;
    background: #252526;
    border-bottom: 1px solid #2a2a2a;
  }
  .tabs button {
    background: none;
    border: none;
    color: #9aa0a6;
    padding: 0 14px;
    cursor: pointer;
    font-size: 12px;
    border-bottom: 2px solid transparent;
  }
  .tabs button.active {
    color: #fff;
    border-bottom-color: #007acc;
  }
  .count {
    margin-left: 5px;
    background: #37373d;
    border-radius: 8px;
    padding: 0 6px;
    font-size: 10px;
  }
  .body {
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: 240px 1fr;
    min-height: 0;
  }
  .sidepanel {
    overflow-y: auto;
    border-right: 1px solid #2a2a2a;
    padding: 4px 0;
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
  .editor {
    flex: 1 1 auto;
    min-height: 0;
    overflow: hidden;
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
