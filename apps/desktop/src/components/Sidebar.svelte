<script lang="ts">
  import { Plus, FolderPlus, ChevronRight, ChevronDown, X, Folder, FolderOpen, FolderCog } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { profile } from "../lib/profiles";
  import type { Tab } from "../lib/types";

  const ws = $derived(app.activeWorkspace);

  function shortPath(p?: string | null): string {
    if (!p) return "(sem pasta)";
    const parts = p.split("/").filter(Boolean);
    return parts.length <= 2 ? "/" + parts.join("/") : "…/" + parts.slice(-2).join("/");
  }

  // ── Drag & drop de abas ──────────────────────────────────────────────────
  function onDragStart(e: DragEvent, tabId: string) {
    e.dataTransfer?.setData("text/plain", tabId);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }
  function dropOnTab(e: DragEvent, target: Tab) {
    e.preventDefault();
    const id = e.dataTransfer?.getData("text/plain");
    if (id) app.moveTab(id, target.folderId ?? null, target.id);
  }
  function dropOnFolder(e: DragEvent, folderId: string | null) {
    e.preventDefault();
    const id = e.dataTransfer?.getData("text/plain");
    if (id) app.moveTab(id, folderId);
  }
  function allowDrop(e: DragEvent) {
    e.preventDefault();
  }
</script>

<div class="sidebar">
  <div class="section-head">
    <span>Workspaces</span>
    <button class="add" title="Novo workspace" onclick={() => app.openNewWorkspaceModal()}><Plus size={15} /></button>
  </div>
  <div class="workspaces">
    {#each app.manifest.workspaces as w (w.id)}
      <div
        class="ws-row"
        class:active={w.id === ws?.id}
        onclick={() => app.selectWorkspace(w.id)}
        ondblclick={() => app.openRenameModal("ws", w.id, w.name)}
        role="button"
        tabindex="0"
      >
        <span class="name">{w.name}</span>
        {#if app.manifest.workspaces.length > 1}
          <button class="mini" title="Remover workspace" onclick={(e) => { e.stopPropagation(); app.deleteWorkspace(w.id); }}><X size={13} /></button>
        {/if}
      </div>
    {/each}
    {#if ws}
      <button class="dirline" title="Trocar a pasta do workspace" onclick={() => app.changeWorkspaceDirectory(ws.id)}>
        <FolderCog size={13} />
        <span>{shortPath(ws.directory)}</span>
      </button>
    {/if}
  </div>

  <div class="section-head">
    <span>Abas</span>
    <button class="add" title="Nova pasta" onclick={() => app.openNewFolderModal()}><FolderPlus size={15} /></button>
  </div>

  <div class="tree" ondragover={allowDrop} ondrop={(e) => dropOnFolder(e, null)}>
    {#if ws}
      {#each ws.folders as folder (folder.id)}
        <div class="folder" ondragover={allowDrop} ondrop={(e) => dropOnFolder(e, folder.id)} role="group">
          <div
            class="folder-head"
            onclick={() => app.toggleFolder(folder.id)}
            ondblclick={(e) => { e.stopPropagation(); app.openRenameModal("folder", folder.id, folder.name); }}
            role="button"
            tabindex="0"
          >
            <span class="caret">
              {#if folder.collapsed}<ChevronRight size={14} />{:else}<ChevronDown size={14} />{/if}
            </span>
            {#if folder.collapsed}<Folder size={14} class="ficon" />{:else}<FolderOpen size={14} class="ficon" />{/if}
            <span class="fname">{folder.name}</span>
            <button class="mini" title="Definir diretório da pasta" onclick={(e) => { e.stopPropagation(); app.changeFolderDirectory(folder.id); }}><FolderCog size={12} /></button>
            <button class="mini" title="Remover pasta" onclick={(e) => { e.stopPropagation(); app.deleteFolder(folder.id); }}><X size={12} /></button>
          </div>
          {#if !folder.collapsed}
            {#each app.tabsInFolder(ws, folder.id) as tab (tab.id)}
              {@render tabRow(tab)}
            {/each}
          {/if}
        </div>
      {/each}

      {#each app.tabsInFolder(ws, null) as tab (tab.id)}
        {@render tabRow(tab)}
      {/each}
    {/if}
  </div>
</div>

{#snippet tabRow(tab: Tab)}
  {@const prof = profile(tab.panes[0]?.toolProfileId ?? "shell")}
  {@const Icon = prof.icon}
  <div
    class="tab-row"
    class:active={tab.id === ws?.activeTabId}
    draggable="true"
    ondragstart={(e) => onDragStart(e, tab.id)}
    ondragover={allowDrop}
    ondrop={(e) => dropOnTab(e, tab)}
    onclick={() => app.selectTab(tab.id)}
    ondblclick={(e) => { e.stopPropagation(); app.openRenameModal("tab", tab.id, tab.title); }}
    role="button"
    tabindex="0"
  >
    <span class="ticon" style="color:{prof.color}"><Icon size={14} /></span>
    <span class="ttitle">{tab.title}</span>
    <button class="mini" title="Fechar aba" onclick={(e) => { e.stopPropagation(); app.closeTab(tab.id); }}><X size={12} /></button>
  </div>
{/snippet}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #202020;
    color: #cccccc;
    font-size: 13px;
    overflow-y: auto;
    user-select: none;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px 4px;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #7a7a7a;
  }
  .add {
    display: flex;
    align-items: center;
    background: none;
    border: none;
    color: #9a9a9a;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
  }
  .add:hover {
    background: #333;
    color: #fff;
  }
  .workspaces {
    padding: 0 6px 6px;
    border-bottom: 1px solid #2a2a2a;
  }
  .ws-row,
  .tab-row,
  .folder-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
  }
  .ws-row:hover,
  .tab-row:hover,
  .folder-head:hover {
    background: #2a2d2e;
  }
  .ws-row.active,
  .tab-row.active {
    background: #37373d;
  }
  .name,
  .ttitle,
  .fname {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dirline {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    background: none;
    border: none;
    color: #6a6a6a;
    cursor: pointer;
    font-size: 11px;
    padding: 3px 8px 2px;
    border-radius: 4px;
    text-align: left;
  }
  .dirline:hover {
    color: #b8b8b8;
    background: #2a2d2e;
  }
  .dirline span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tree {
    flex: 1 1 auto;
    padding: 4px 6px;
    min-height: 40px;
  }
  .tab-row {
    margin-left: 6px;
  }
  .folder :global(.ficon) {
    color: #8a8a8a;
    flex: 0 0 auto;
  }
  .folder .tab-row {
    margin-left: 14px;
  }
  .caret {
    display: flex;
    align-items: center;
    color: #8a8a8a;
    flex: 0 0 auto;
  }
  .ticon {
    display: flex;
    flex: 0 0 auto;
  }
  .mini {
    display: flex;
    align-items: center;
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    padding: 2px;
    border-radius: 3px;
    opacity: 0;
  }
  .ws-row:hover .mini,
  .tab-row:hover .mini,
  .folder-head:hover .mini {
    opacity: 1;
  }
  .mini:hover {
    color: #eee;
    background: #4a4a4a;
  }
</style>
