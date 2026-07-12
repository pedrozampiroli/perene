<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./lib/store.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import TabGrid from "./components/TabGrid.svelte";
  import TopBar from "./components/TopBar.svelte";
  import BottomBar from "./components/BottomBar.svelte";
  import SettingsModal from "./components/SettingsModal.svelte";
  import HistoryModal from "./components/HistoryModal.svelte";
  import UsageModal from "./components/UsageModal.svelte";
  import NameModal from "./components/NameModal.svelte";
  import ConfirmModal from "./components/ConfirmModal.svelte";

  const isMac = navigator.userAgent.toLowerCase().includes("mac");

  onMount(() => {
    void app.load();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function onKey(e: KeyboardEvent) {
    // Não sequestra atalhos enquanto renomeia (input); o terminal usa <textarea>.
    if ((e.target as HTMLElement)?.tagName === "INPUT") return;
    const primary = isMac ? e.metaKey : e.ctrlKey;
    if (!primary) return;
    const k = e.key.toLowerCase();

    if (k === ",") {
      e.preventDefault();
      app.settingsOpen = !app.settingsOpen;
      return;
    }
    const group = isMac ? !e.shiftKey : e.shiftKey;
    if (group && k === "y") {
      e.preventDefault();
      app.historyOpen = !app.historyOpen;
      return;
    }
    if (group && k === "u") {
      e.preventDefault();
      app.usageOpen = !app.usageOpen;
      return;
    }
    if (k >= "1" && k <= "9" && !(!isMac && e.shiftKey)) {
      const ws = app.activeWorkspace;
      const tab = ws?.tabs[Number(k) - 1];
      if (tab) {
        e.preventDefault();
        app.selectTab(tab.id);
      }
      return;
    }
    // Grupo T/W/D: mac = só Cmd; win/linux = Ctrl+Shift (evita colidir com o PTY).
    if (group && k === "t") {
      e.preventDefault();
      app.createTab("shell");
      return;
    }
    if (group && k === "w") {
      e.preventDefault();
      if (app.activePaneId) app.closePane(app.activePaneId);
      return;
    }
    if (group && k === "d") {
      e.preventDefault();
      if (app.activePaneId) app.splitPane(app.activePaneId, "horizontal", "shell");
      return;
    }
    // Dividir abaixo: mac Cmd+Shift+D / win-linux Ctrl+Alt+D.
    if (k === "d" && ((isMac && e.shiftKey) || (!isMac && e.altKey))) {
      e.preventDefault();
      if (app.activePaneId) app.splitPane(app.activePaneId, "vertical", "shell");
    }
  }
</script>

{#if app.loaded}
  <div class="app">
    <div class="sidebar-col"><Sidebar /></div>
    <div class="topbar-col"><TopBar /></div>
    <div class="main"><TabGrid /></div>
    <div class="bottom"><BottomBar /></div>
  </div>
  {#if app.settingsOpen}
    <SettingsModal />
  {/if}
  {#if app.historyOpen}
    <HistoryModal />
  {/if}
  {#if app.usageOpen}
    <UsageModal />
  {/if}
  <NameModal />
  <ConfirmModal />
{:else}
  <div class="splash">Perene…</div>
{/if}

<style>
  .app {
    display: grid;
    grid-template-columns: 240px 1fr;
    grid-template-rows: 32px 1fr 34px;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
  .sidebar-col {
    grid-row: 1 / 3;
    grid-column: 1;
    border-right: 1px solid #2a2a2a;
    min-height: 0;
  }
  .topbar-col {
    grid-row: 1;
    grid-column: 2;
    min-width: 0;
  }
  .main {
    grid-row: 2;
    grid-column: 2;
    min-width: 0;
    min-height: 0;
    background: #1e1e1e;
  }
  .bottom {
    grid-row: 3;
    grid-column: 1 / 3;
  }
  .splash {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    color: #6a6a6a;
  }
</style>
