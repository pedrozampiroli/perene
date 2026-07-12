<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { X, Folder } from "@lucide/svelte";
  import { PerenePane } from "../lib/terminal";
  import { app } from "../lib/store.svelte";
  import { profile } from "../lib/profiles";
  import FilesPane from "./FilesPane.svelte";

  let { paneId }: { paneId: string } = $props();

  let container: HTMLDivElement;
  let pane: PerenePane | undefined;

  const data = $derived(app.findPane(paneId));
  const isFiles = $derived(data?.kind === "files");
  const prof = $derived(profile(data?.toolProfileId ?? "shell"));
  const HeadIcon = $derived(isFiles ? Folder : prof.icon);
  const isActive = $derived(app.activePaneId === paneId);
  const dirLabel = $derived((data?.workingDirectory ?? "").split("/").filter(Boolean).pop() ?? "~");

  onMount(() => {
    const p = app.findPane(paneId);
    if (!p || p.kind === "files") return; // pane de arquivos não abre PTY
    pane = new PerenePane(paneId, app.settings.fontSize);
    pane
      .open(container, {
        cwd: p.workingDirectory,
        command: app.commandFor(p),
        fontSize: app.settings.fontSize,
        webgl: app.settings.webgl,
        shell: app.settings.shell || null,
      })
      .catch(() => {}); // erros de spawn não devem virar unhandledrejection
  });
  onDestroy(() => pane?.dispose());

  $effect(() => {
    if (isActive && !isFiles) pane?.focus();
  });

  function focusPane() {
    app.setActivePane(paneId);
    if (!isFiles) pane?.focus();
  }
</script>

<div class="pane" class:active={isActive} onpointerdown={focusPane}>
  <div class="pane-head" style="--accent:{isFiles ? '#6ea8fe' : prof.color}">
    <span class="hicon" style="color:{isFiles ? '#6ea8fe' : prof.color}"><HeadIcon size={13} /></span>
    <span class="label">{isFiles ? "Arquivos" : prof.label}</span>
    <span class="dir">{dirLabel}</span>
    <button class="x" title="Fechar painel (⌘W)" onclick={() => app.closePane(paneId)}><X size={13} /></button>
  </div>
  {#if isFiles}
    <div class="term"><FilesPane {paneId} /></div>
  {:else}
    <div class="term" bind:this={container}></div>
  {/if}
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: #1e1e1e;
    border: 1px solid transparent;
    box-sizing: border-box;
    overflow: hidden;
  }
  .pane.active {
    border-color: #3a3d41;
  }
  .pane-head {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    padding: 0 8px;
    font-size: 11px;
    color: #9aa0a6;
    background: #252526;
    border-bottom: 1px solid #2a2a2a;
    flex: 0 0 auto;
    user-select: none;
  }
  .hicon {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
  }
  .label {
    color: #cccccc;
    font-weight: 600;
  }
  .dir {
    color: #6a6a6a;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .x {
    display: flex;
    align-items: center;
    margin-left: auto;
    background: none;
    border: none;
    color: #6a6a6a;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
  }
  .x:hover {
    color: #ddd;
    background: #3a3d41;
  }
  .term {
    flex: 1 1 auto;
    min-height: 0;
    padding: 4px 6px;
    box-sizing: border-box;
  }
</style>
