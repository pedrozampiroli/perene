<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { PTY_STATUS } from "./lib/events";
  import type { PaneState } from "./lib/types";
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
  import NewSessionModal from "./components/NewSessionModal.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import SearchPalette from "./components/SearchPalette.svelte";
  import Onboarding from "./components/Onboarding.svelte";

  const isMac = navigator.userAgent.toLowerCase().includes("mac");

  // ── Sidebar redimensionável ────────────────────────────────────────────────
  let resizingSidebar = $state(false);
  function startSidebarResize(e: PointerEvent) {
    e.preventDefault();
    resizingSidebar = true;
    const move = (ev: PointerEvent) => app.setSidebarWidth(ev.clientX);
    const up = () => {
      resizingSidebar = false;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  onMount(() => {
    void app.load();
    // Indicador de status das sessões: um listener global (o daemon só emite
    // quando o estado MUDA, então é barato).
    const un = listen<{ paneId: string; state: PaneState }>(PTY_STATUS, (e) =>
      app.setPaneStatus(e.payload.paneId, e.payload.state),
    );
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      void un.then((f) => f());
    };
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
    // Busca (estilo VSCodium). ⌘F/⌘H dentro do editor são do CodeMirror.
    if (k === "p" && !e.shiftKey) {
      e.preventDefault();
      void app.openQuickOpen();
      return;
    }
    if (k === "f" && e.shiftKey) {
      e.preventDefault();
      app.openGlobalSearch();
      return;
    }
    if (k === "h" && e.shiftKey) {
      e.preventDefault();
      app.openGlobalReplace();
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
      app.startNewSession("shell");
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
  <div class="app" style="--sw:{app.settings.sidebarWidth}px">
    <div class="sidebar-col"><Sidebar /></div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="vdivider" class:dragging={resizingSidebar} onpointerdown={startSidebarResize}></div>
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
  <NewSessionModal />
  <ContextMenu />
  <SearchPalette />
  <Onboarding />
{:else}
  <div class="splash">Perene…</div>
{/if}

<style>
  .app {
    display: grid;
    grid-template-columns: var(--sw, 240px) 1px 1fr;
    grid-template-rows: 32px 1fr 34px;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
  .sidebar-col {
    grid-row: 1 / 3;
    grid-column: 1;
    min-height: 0;
    min-width: 0;
  }
  /* Divisor arrastável da sidebar (área de clique maior que a linha). */
  .vdivider {
    grid-row: 1 / 3;
    grid-column: 2;
    position: relative;
    background: #2a2a2a;
    cursor: col-resize;
  }
  .vdivider::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: -3px;
    right: -3px;
  }
  .vdivider:hover,
  .vdivider.dragging {
    background: #007acc;
  }
  .topbar-col {
    grid-row: 1;
    grid-column: 3;
    min-width: 0;
  }
  .main {
    grid-row: 2;
    grid-column: 3;
    min-width: 0;
    min-height: 0;
    background: #1e1e1e;
  }
  .bottom {
    grid-row: 3;
    grid-column: 1 / 4;
  }
  .splash {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    color: #6a6a6a;
  }
</style>
