<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { PROFILES, profile } from "../lib/profiles";
  import type { SplitDirection } from "../lib/types";

  const ws = $derived(app.activeWorkspace);

  function split(dir: SplitDirection) {
    if (app.activePaneId) app.splitPane(app.activePaneId, dir, "shell");
  }
</script>

<div class="bottombar">
  <!-- Novo terminal por perfil (ícone + cor) -->
  <div class="group profiles">
    {#each PROFILES as p (p.id)}
      <button
        class="prof"
        style="--c:{p.color}"
        title={"Novo " + p.label}
        onclick={() => app.createTab(p.id)}
      >
        <span class="ic">{p.icon}</span>
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

  <!-- Splits + presets de layout -->
  <div class="group">
    <button title="Dividir à direita (⌘D)" onclick={() => split("horizontal")}>⇥</button>
    <button title="Dividir abaixo (⌘⇧D)" onclick={() => split("vertical")}>⤓</button>
    <button title="Colunas" onclick={() => app.arrange("columns")}>▥</button>
    <button title="Linhas" onclick={() => app.arrange("rows")}>▤</button>
    <button title="Grade" onclick={() => app.arrange("grid")}>▦</button>
  </div>

  <button class="gear" title="Configurações (⌘,)" onclick={() => (app.settingsOpen = true)}>⚙</button>
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
    gap: 3px;
  }
  .tabs {
    overflow-x: auto;
    max-width: 45vw;
  }
  .spacer {
    flex: 1 1 auto;
  }
  button {
    background: none;
    border: none;
    color: #b8b8b8;
    cursor: pointer;
    border-radius: 4px;
    padding: 4px 7px;
    font-size: 13px;
    line-height: 1;
    white-space: nowrap;
  }
  button:hover {
    background: #37373d;
    color: #fff;
  }
  .prof .ic {
    color: var(--c);
    font-size: 14px;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 5px;
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
  .gear {
    font-size: 15px;
  }
</style>
