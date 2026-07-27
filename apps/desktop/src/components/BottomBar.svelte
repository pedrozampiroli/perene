<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import {
    SquareSplitHorizontal,
    SquareSplitVertical,
    Columns3,
    Rows3,
    LayoutGrid,
    Code2,
    History,
    ChartColumn,
    Settings,
  } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { PROFILES, profile } from "../lib/profiles";
  import ToolIcon from "./ToolIcon.svelte";
  import type { SplitDirection } from "../lib/types";

  const ws = $derived(app.activeWorkspace);

  function split(dir: SplitDirection) {
    if (app.activePaneId) app.splitPane(app.activePaneId, dir, "shell");
  }
</script>

<div class="bottombar">
  <!-- Novo terminal por perfil -->
  <div class="group profiles">
    {#each PROFILES as p (p.id)}
      <button class="prof" style="--c:{p.color}" title={t("bottom.newSession", { tool: p.label })} onclick={() => app.startNewSession(p.id)}>
        <ToolIcon id={p.id} size={17} />
      </button>
    {/each}
  </div>

  <!-- Tira de abas do workspace ativo -->
  <div class="group tabs">
    {#if ws}
      {#each ws.tabs as tab (tab.id)}
        <button
          class="tab"
          class:active={tab.id === ws.activeTabId}
          style="--c:{profile(tab.panes[0]?.toolProfileId ?? 'shell').color}"
          onclick={() => app.selectTab(tab.id)}
          oncontextmenu={(e) => app.openContextMenu(e, app.tabMenu(tab.id))}
          title={tab.title}
        >
          <span class="ti"><ToolIcon id={tab.panes[0]?.toolProfileId ?? "shell"} size={13} /></span>{tab.title}
        </button>
      {/each}
    {/if}
  </div>

  <div class="spacer"></div>

  <div class="group">
    <button title={t("bottom.splitRight") + " (⌘D)"} onclick={() => split("horizontal")}><SquareSplitHorizontal size={16} /></button>
    <button title={t("bottom.splitDown") + " (⌘⇧D)"} onclick={() => split("vertical")}><SquareSplitVertical size={16} /></button>
    <button title={t("bottom.columns")} onclick={() => app.arrange("columns")}><Columns3 size={16} /></button>
    <button title={t("bottom.rows")} onclick={() => app.arrange("rows")}><Rows3 size={16} /></button>
    <button title={t("bottom.grid")} onclick={() => app.arrange("grid")}><LayoutGrid size={16} /></button>
  </div>

  <div class="sep"></div>

  <div class="group">
    <button title={t("bottom.fileEditor")} onclick={() => app.openFilesTab()}><Code2 size={16} /></button>
    <button title={t("bottom.history") + " (⌘Y)"} onclick={() => (app.historyOpen = true)}><History size={16} /></button>
    <button title={t("bottom.usage") + " (⌘U)"} onclick={() => (app.usageOpen = true)}><ChartColumn size={16} /></button>
    <button title={t("bottom.settings") + " (⌘,)"} onclick={() => (app.settingsOpen = true)}><Settings size={16} /></button>
  </div>
</div>

<style>
  .bottombar {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    padding: 0 8px;
    background: #252526;
    border-top: 1px solid #2a2a2a;
    overflow: hidden;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .sep {
    width: 1px;
    height: 18px;
    background: #3a3a3a;
    margin: 0 2px;
  }
  .tabs {
    overflow-x: auto;
    max-width: 42vw;
  }
  .spacer {
    flex: 1 1 auto;
  }
  button {
    display: flex;
    align-items: center;
    gap: 5px;
    background: none;
    border: none;
    color: #b8b8b8;
    cursor: pointer;
    border-radius: 5px;
    padding: 5px 7px;
    font-size: 12px;
    line-height: 1;
    white-space: nowrap;
  }
  button:hover {
    background: #37373d;
    color: #fff;
  }
  .prof {
    color: var(--c);
  }
  .prof:hover {
    background: #37373d;
  }
  .tab {
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tab .ti {
    display: flex;
    color: var(--c);
    flex: 0 0 auto;
  }
  .tab.active {
    background: #37373d;
    color: #fff;
  }
</style>
