<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { profile } from "../lib/profiles";
  import type { Tab } from "../lib/types";

  const ws = $derived(app.activeWorkspace);

  type Editing = { type: "ws" | "folder" | "tab"; id: string } | null;
  let editing = $state<Editing>(null);
  let editValue = $state("");

  function startEdit(type: "ws" | "folder" | "tab", id: string, current: string) {
    editing = { type, id };
    editValue = current;
  }
  function commitEdit() {
    if (!editing) return;
    const v = editValue.trim();
    if (v) {
      if (editing.type === "ws") app.renameWorkspace(editing.id, v);
      else if (editing.type === "folder") app.renameFolder(editing.id, v);
      else app.renameTab(editing.id, v);
    }
    editing = null;
  }
  function onEditKey(e: KeyboardEvent) {
    if (e.key === "Enter") commitEdit();
    else if (e.key === "Escape") editing = null;
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
    <button class="add" title="Novo workspace" onclick={() => app.createWorkspace()}>+</button>
  </div>
  <div class="workspaces">
    {#each app.manifest.workspaces as w (w.id)}
      <div
        class="ws-row"
        class:active={w.id === ws?.id}
        onclick={() => app.selectWorkspace(w.id)}
        ondblclick={() => startEdit("ws", w.id, w.name)}
        role="button"
        tabindex="0"
      >
        {#if editing?.type === "ws" && editing.id === w.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            bind:value={editValue}
            onblur={commitEdit}
            onkeydown={onEditKey}
            autofocus
          />
        {:else}
          <span class="name">{w.name}</span>
          {#if app.manifest.workspaces.length > 1}
            <button
              class="del"
              title="Remover workspace"
              onclick={(e) => {
                e.stopPropagation();
                app.deleteWorkspace(w.id);
              }}>✕</button
            >
          {/if}
        {/if}
      </div>
    {/each}
  </div>

  <div class="section-head">
    <span>Abas</span>
    <button class="add" title="Nova pasta" onclick={() => app.createFolder()}>⊞</button>
  </div>

  <!-- Área raiz (drop = tirar da pasta) -->
  <div class="tree" ondragover={allowDrop} ondrop={(e) => dropOnFolder(e, null)}>
    {#if ws}
      {#each ws.folders as folder (folder.id)}
        <div
          class="folder"
          ondragover={allowDrop}
          ondrop={(e) => dropOnFolder(e, folder.id)}
          role="group"
        >
          <div class="folder-head" onclick={() => app.toggleFolder(folder.id)} role="button" tabindex="0">
            <span class="caret">{folder.collapsed ? "▸" : "▾"}</span>
            {#if editing?.type === "folder" && editing.id === folder.id}
              <!-- svelte-ignore a11y_autofocus -->
              <input bind:value={editValue} onblur={commitEdit} onkeydown={onEditKey} autofocus onclick={(e) => e.stopPropagation()} />
            {:else}
              <span
                class="fname"
                ondblclick={(e) => {
                  e.stopPropagation();
                  startEdit("folder", folder.id, folder.name);
                }}>{folder.name}</span
              >
              <button class="del" title="Remover pasta" onclick={(e) => { e.stopPropagation(); app.deleteFolder(folder.id); }}>✕</button>
            {/if}
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
  <div
    class="tab-row"
    class:active={tab.id === ws?.activeTabId}
    draggable="true"
    ondragstart={(e) => onDragStart(e, tab.id)}
    ondragover={allowDrop}
    ondrop={(e) => dropOnTab(e, tab)}
    onclick={() => app.selectTab(tab.id)}
    role="button"
    tabindex="0"
  >
    <span class="dot" style="background:{profile(tab.panes[0]?.toolProfileId ?? 'shell').color}"></span>
    {#if editing?.type === "tab" && editing.id === tab.id}
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={editValue} onblur={commitEdit} onkeydown={onEditKey} autofocus onclick={(e) => e.stopPropagation()} />
    {:else}
      <span class="ttitle" ondblclick={(e) => { e.stopPropagation(); startEdit("tab", tab.id, tab.title); }}>{tab.title}</span>
      <button class="del" title="Fechar aba" onclick={(e) => { e.stopPropagation(); app.closeTab(tab.id); }}>✕</button>
    {/if}
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
    background: none;
    border: none;
    color: #9a9a9a;
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
    padding: 2px 6px;
    border-radius: 3px;
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
  .tree {
    flex: 1 1 auto;
    padding: 4px 6px;
    min-height: 40px;
  }
  .tab-row {
    margin-left: 6px;
  }
  .folder .tab-row {
    margin-left: 16px;
  }
  .caret {
    width: 12px;
    color: #8a8a8a;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: 0 0 auto;
  }
  .del {
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 10px;
    padding: 2px 4px;
    border-radius: 3px;
    opacity: 0;
  }
  .ws-row:hover .del,
  .tab-row:hover .del,
  .folder-head:hover .del {
    opacity: 1;
  }
  .del:hover {
    color: #eee;
    background: #4a4a4a;
  }
  input {
    flex: 1 1 auto;
    background: #1e1e1e;
    border: 1px solid #007acc;
    color: #fff;
    font-size: 13px;
    padding: 2px 4px;
    border-radius: 3px;
    outline: none;
    min-width: 0;
  }
</style>
