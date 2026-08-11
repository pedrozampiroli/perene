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
  <div data-tour="git"><GitWidget /></div>
</div>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 100%;
    padding: 0 10px;
    background: var(--bg);
    border-bottom: 1px solid var(--elevated);
    /* sem overflow:hidden — senão corta o menu git que abre pra baixo */
  }
  .crumb {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    font-size: 12px;
    color: var(--muted);
  }
  .ws {
    color: var(--muted);
  }
  .sl {
    color: var(--border);
  }
  .ic {
    display: flex;
    flex: 0 0 auto;
  }
  .tb {
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1 1 auto;
  }
</style>
