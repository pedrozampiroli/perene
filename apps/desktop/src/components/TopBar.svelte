<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { profile } from "../lib/profiles";
  import GitWidget from "./GitWidget.svelte";
  import ToolIcon from "./ToolIcon.svelte";

  const tab = $derived(app.activeTab);
  const prof = $derived(profile(tab?.panes[0]?.toolProfileId ?? "shell"));
</script>

<div class="topbar">
  <div class="crumb">
    {#if app.activeWorkspace}
      <span class="ws">{app.activeWorkspace.name}</span>
    {/if}
    {#if tab}
      <span class="sl">›</span>
      <span class="ic" style="color:{prof.color}"><ToolIcon id={tab.panes[0]?.toolProfileId ?? "shell"} size={13} /></span>
      <span class="tb">{tab.title}</span>
    {/if}
  </div>
  <div class="spacer"></div>
  <GitWidget />
</div>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 100%;
    padding: 0 10px;
    background: #1e1e1e;
    border-bottom: 1px solid #2a2a2a;
    /* sem overflow:hidden — senão corta o menu git que abre pra baixo */
  }
  .crumb {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    font-size: 12px;
    color: #b8b8b8;
  }
  .ws {
    color: #9aa0a6;
  }
  .sl {
    color: #5a5a5a;
  }
  .ic {
    display: flex;
    flex: 0 0 auto;
  }
  .tb {
    color: #e0e0e0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1 1 auto;
  }
</style>
