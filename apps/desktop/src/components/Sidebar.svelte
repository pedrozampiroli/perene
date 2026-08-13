<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { Plus, FolderPlus, ChevronRight, ChevronDown, X, Folder, FolderOpen, FolderCog } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { profile } from "../lib/profiles";
  import { shortPath as shortenPath } from "../lib/paths";
  import ToolIcon from "./ToolIcon.svelte";
  import StatusDot from "./StatusDot.svelte";
  import type { Tab, Folder as FolderType } from "../lib/types";

  const ws = $derived(app.activeWorkspace);

  function shortPath(p?: string | null): string {
    if (!p) return t("sidebar.noDirectory");
    return shortenPath(p);
  }

  // ── Drag & drop de abas e pastas ────────────────────────────────────────
  // IMPORTANTE: os handlers de drop precisam de stopPropagation, senão o evento
  // borbulha até a raiz (.tree) e a aba/pasta acaba caindo fora do alvo.
  type DragItem = { kind: "tab" | "folder"; id: string };
  let dragging = $state<DragItem | null>(null);
  let overFolder = $state<string | null>(null); // id da pasta (ou "__root__")

  function onDragStart(e: DragEvent, kind: DragItem["kind"], id: string) {
    dragging = { kind, id };
    e.dataTransfer?.setData("text/plain", id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }
  function onDragEnd() {
    dragging = null;
    overFolder = null;
  }
  /** Uma pasta não pode cair dentro dela mesma nem de uma subpasta dela. */
  function folderDropAllowed(folderId: string | null): boolean {
    if (!dragging || dragging.kind !== "folder") return true;
    if (folderId === null) return true;
    return ws ? !app.isFolderOrDescendant(ws, dragging.id, folderId) : true;
  }
  function allowDrop(e: DragEvent, folderId: string | null) {
    e.preventDefault();
    e.stopPropagation();
    if (!folderDropAllowed(folderId)) {
      if (e.dataTransfer) e.dataTransfer.dropEffect = "none";
      overFolder = null;
      return;
    }
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    overFolder = folderId ?? "__root__";
  }
  function dropOnFolder(e: DragEvent, folderId: string | null) {
    e.preventDefault();
    e.stopPropagation();
    const item = dragging ?? { kind: "tab" as const, id: e.dataTransfer?.getData("text/plain") ?? "" };
    if (item.id && folderDropAllowed(folderId)) {
      if (item.kind === "folder") app.moveFolder(item.id, folderId);
      else app.moveTab(item.id, folderId);
    }
    onDragEnd();
  }
  function dropOnTab(e: DragEvent, target: Tab) {
    e.preventDefault();
    e.stopPropagation();
    const item = dragging ?? { kind: "tab" as const, id: e.dataTransfer?.getData("text/plain") ?? "" };
    if (!item.id) return onDragEnd();
    if (item.kind === "folder") app.moveFolder(item.id, target.folderId ?? null);
    else if (item.id !== target.id) app.moveTab(item.id, target.folderId ?? null, target.id);
    onDragEnd();
  }
</script>

<div class="sidebar">
  <div class="section-head">
    <span>{t("sidebar.workspaces")}</span>
    <button class="add" title={t("sidebar.newWorkspace")} onclick={() => app.openNewWorkspaceModal()}><Plus size={15} /></button>
  </div>
  <div class="workspaces" data-tour="workspaces">
    {#each app.manifest.workspaces as w (w.id)}
      <div
        class="ws-row"
        class:active={w.id === ws?.id}
        onclick={() => app.selectWorkspace(w.id)}
        ondblclick={() => app.openRenameModal("ws", w.id, w.name)}
        oncontextmenu={(e) => app.openContextMenu(e, app.workspaceMenu(w.id))}
        role="button"
        tabindex="0"
      >
        <span class="name">{w.name}</span>
        <span class="wstatus"><StatusDot state={app.workspaceStatus(w)} size={7} /></span>
        {#if app.manifest.workspaces.length > 1}
          <button class="mini" title={t("sidebar.removeWorkspace")} onclick={(e) => { e.stopPropagation(); app.confirmDeleteWorkspace(w.id); }}><X size={13} /></button>
        {/if}
      </div>
    {/each}
    {#if ws}
      <button class="dirline" title={t("sidebar.changeWorkspaceDirectory")} onclick={() => app.changeWorkspaceDirectory(ws.id)}>
        <FolderCog size={13} />
        <span>{shortPath(ws.directory)}</span>
      </button>
    {/if}
  </div>

  <div class="section-head">
    <span>{t("sidebar.tabs")}</span>
    <button class="add" title={t("sidebar.newFolder")} onclick={() => app.openNewFolderModal()}><FolderPlus size={15} /></button>
  </div>

  <div
    class="tree"
    data-tour="tabs"
    class:over={overFolder === "__root__"}
    ondragover={(e) => allowDrop(e, null)}
    ondragleave={() => (overFolder = null)}
    ondrop={(e) => dropOnFolder(e, null)}
    oncontextmenu={(e) => ws && app.openContextMenu(e, app.workspaceMenu(ws.id))}
    role="tree"
    tabindex="-1"
  >
    {#if ws}
      {#each app.foldersInFolder(ws, null) as folder (folder.id)}
        {@render folderNode(folder)}
      {/each}

      {#each app.tabsInFolder(ws, null) as tab (tab.id)}
        {@render tabRow(tab)}
      {/each}
    {/if}
  </div>
</div>

{#snippet folderNode(folder: FolderType)}
  {@const children = ws ? app.foldersInFolder(ws, folder.id) : []}
  {@const tabs = ws ? app.tabsInFolder(ws, folder.id) : []}
  <div
    class="folder"
    class:over={overFolder === folder.id}
    ondragover={(e) => allowDrop(e, folder.id)}
    ondragleave={(e) => { e.stopPropagation(); overFolder = null; }}
    ondrop={(e) => dropOnFolder(e, folder.id)}
    role="group"
  >
    <div
      class="folder-head"
      class:drag={dragging?.kind === "folder" && dragging.id === folder.id}
      draggable="true"
      ondragstart={(e) => onDragStart(e, "folder", folder.id)}
      ondragend={onDragEnd}
      onclick={() => app.toggleFolder(folder.id)}
      ondblclick={(e) => { e.stopPropagation(); app.openRenameModal("folder", folder.id, folder.name); }}
      oncontextmenu={(e) => app.openContextMenu(e, app.folderMenu(folder.id))}
      role="button"
      tabindex="0"
    >
      <span class="caret">
        {#if folder.collapsed}<ChevronRight size={14} />{:else}<ChevronDown size={14} />{/if}
      </span>
      {#if folder.collapsed}<Folder size={14} class="ficon" />{:else}<FolderOpen size={14} class="ficon" />{/if}
      <span class="fname">{folder.name}</span>
      <span class="fcount">{ws ? app.tabsInSubtree(ws, folder.id).length : 0}</span>
      <span class="fstatus"><StatusDot state={ws ? app.folderStatus(ws, folder.id) : null} size={7} /></span>
      <button
        class="mini"
        title={t("sidebar.newSessionInFolder")}
        onclick={(e) => { e.stopPropagation(); app.openContextMenu(e, app.folderMenu(folder.id)); }}
      ><Plus size={12} /></button>
      <button class="mini" title={t("sidebar.setFolderDirectory")} onclick={(e) => { e.stopPropagation(); app.changeFolderDirectory(folder.id); }}><FolderCog size={12} /></button>
      <button class="mini" title={t("sidebar.removeFolder")} onclick={(e) => { e.stopPropagation(); app.confirmDeleteFolder(folder.id); }}><X size={12} /></button>
    </div>
    {#if !folder.collapsed}
      {#each children as child (child.id)}
        {@render folderNode(child)}
      {/each}
      {#each tabs as tab (tab.id)}
        {@render tabRow(tab)}
      {/each}
      {#if !children.length && !tabs.length}
        <div class="fempty">{t("sidebar.dropHint")}</div>
      {/if}
    {/if}
  </div>
{/snippet}

{#snippet tabRow(tab: Tab)}
  {@const prof = profile(tab.panes[0]?.toolProfileId ?? "shell")}
  <div
    class="tab-row"
    class:active={tab.id === ws?.activeTabId}
    class:drag={dragging?.kind === "tab" && dragging.id === tab.id}
    draggable="true"
    ondragstart={(e) => onDragStart(e, "tab", tab.id)}
    ondragend={onDragEnd}
    ondragover={(e) => allowDrop(e, tab.folderId ?? null)}
    ondrop={(e) => dropOnTab(e, tab)}
    onclick={() => app.selectTab(tab.id)}
    ondblclick={(e) => { e.stopPropagation(); app.openRenameModal("tab", tab.id, tab.title); }}
    oncontextmenu={(e) => app.openContextMenu(e, app.tabMenu(tab.id))}
    role="button"
    tabindex="0"
  >
    <span class="ticon" style="color:{prof.color}"><ToolIcon id={tab.panes[0]?.toolProfileId ?? "shell"} size={14} /></span>
    <span class="ttitle">{tab.title}</span>
    <span class="tstatus" style="color:{prof.color}"><StatusDot state={app.tabStatus(tab)} /></span>
    <button class="mini" title={t("sidebar.closeTab")} onclick={(e) => { e.stopPropagation(); app.confirmCloseTab(tab.id); }}><X size={12} /></button>
  </div>
{/snippet}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg);
    color: var(--fg);
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
    color: var(--muted);
  }
  .add {
    display: flex;
    align-items: center;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 4px;
  }
  .add:hover {
    background: var(--border);
    color: var(--fg);
  }
  .workspaces {
    padding: 0 6px 6px;
    border-bottom: 1px solid var(--elevated);
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
    background: var(--elevated);
  }
  .ws-row.active,
  .tab-row.active {
    background: var(--elevated);
  }
  .tab-row.drag {
    opacity: 0.45;
  }
  .name,
  .ttitle,
  .fname {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Indicadores de workspace e pasta: reservam a própria coluna para o nome não
     dançar quando o estado aparece/some. */
  .wstatus,
  .fstatus {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    width: 9px;
    margin-left: auto;
  }
  .fstatus {
    margin-left: 0;
  }
  .fcount {
    font-size: 10px;
    color: var(--muted);
    background: var(--elevated);
    border-radius: 8px;
    padding: 0 5px;
  }
  .dirline {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 11px;
    padding: 3px 8px 2px;
    border-radius: 4px;
    text-align: left;
  }
  .dirline:hover {
    color: var(--muted);
    background: var(--elevated);
  }
  .dirline span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tree {
    flex: 1 1 auto;
    padding: 4px 6px;
    min-height: 60px;
    border: 1px dashed transparent;
    border-radius: 6px;
  }
  /* Feedback de onde a aba vai cair. */
  .tree.over {
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
    background: color-mix(in srgb, var(--accent) 6%, transparent);
  }
  .folder {
    border: 1px dashed transparent;
    border-radius: 6px;
  }
  .folder.over {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .fempty {
    font-size: 11px;
    color: var(--border);
    padding: 3px 8px 5px 30px;
    font-style: italic;
  }
  .tab-row {
    margin-left: 6px;
  }
  .folder :global(.ficon) {
    color: var(--muted);
    flex: 0 0 auto;
  }
  .folder .tab-row {
    margin-left: 14px;
  }
  /* Pasta dentro de pasta: cada nível soma seu próprio recuo — o efeito de
     árvore vem de aplicar o mesmo recuo em cada aninhamento, não de calcular
     a profundidade em JS. */
  .folder .folder {
    margin-left: 14px;
  }
  .folder-head.drag {
    opacity: 0.45;
  }
  .caret {
    display: flex;
    align-items: center;
    color: var(--muted);
    flex: 0 0 auto;
  }
  .ticon {
    display: flex;
    flex: 0 0 auto;
  }
  /* Indicador some junto com os botões no hover pra não competir com eles. */
  .tstatus {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
  }
  .tab-row:hover .tstatus {
    display: none;
  }
  .mini {
    display: flex;
    align-items: center;
    background: none;
    border: none;
    color: var(--muted);
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
    color: var(--fg);
    background: var(--border);
  }
</style>
