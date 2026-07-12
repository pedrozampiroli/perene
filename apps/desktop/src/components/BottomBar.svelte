<script lang="ts">
  import {
    SquareSplitHorizontal,
    SquareSplitVertical,
    Columns3,
    Rows3,
    LayoutGrid,
    FolderTree,
    History,
    ChartColumn,
    Settings,
  } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { PROFILES, profile } from "../lib/profiles";
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
      {@const Icon = p.icon}
      <button class="prof" style="--c:{p.color}" title={"Novo " + p.label} onclick={() => app.createTab(p.id)}>
        <Icon size={16} />
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
          title={tab.title}
        >
          <span class="d"></span>{tab.title}
        </button>
      {/each}
    {/if}
  </div>

  <div class="spacer"></div>

  <div class="group">
    <button title="Dividir à direita (⌘D)" onclick={() => split("horizontal")}><SquareSplitHorizontal size={16} /></button>
    <button title="Dividir abaixo (⌘⇧D)" onclick={() => split("vertical")}><SquareSplitVertical size={16} /></button>
    <button title="Colunas" onclick={() => app.arrange("columns")}><Columns3 size={16} /></button>
    <button title="Linhas" onclick={() => app.arrange("rows")}><Rows3 size={16} /></button>
    <button title="Grade" onclick={() => app.arrange("grid")}><LayoutGrid size={16} /></button>
  </div>

  <div class="sep"></div>

  <div class="group">
    <button title="Visualizador de arquivos" onclick={() => app.openFilesTab()}><FolderTree size={16} /></button>
    <button title="Histórico de sessões (⌘Y)" onclick={() => (app.historyOpen = true)}><History size={16} /></button>
    <button title="Uso de tokens (⌘U)" onclick={() => (app.usageOpen = true)}><ChartColumn size={16} /></button>
    <button title="Configurações (⌘,)" onclick={() => (app.settingsOpen = true)}><Settings size={16} /></button>
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
  .tab .d {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--c);
    flex: 0 0 auto;
  }
  .tab.active {
    background: #37373d;
    color: #fff;
  }
</style>
