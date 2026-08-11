<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { onMount, onDestroy } from "svelte";
  import { X, Code2 } from "@lucide/svelte";
  import { isPermissionGranted, sendNotification } from "@tauri-apps/plugin-notification";
  import { PerenePane } from "../lib/terminal";
  import { app } from "../lib/store.svelte";
  import { profile } from "../lib/profiles";
  import { baseName } from "../lib/paths";
  import FilesPane from "./FilesPane.svelte";
  import AcpPane from "./AcpPane.svelte";
  import ToolIcon from "./ToolIcon.svelte";
  import StatusDot from "./StatusDot.svelte";

  /** Bells em rajada (ex.: vários BEL no mesmo chunk) viram 1 notificação só. */
  const BELL_DEBOUNCE_MS = 4000;

  let { paneId }: { paneId: string } = $props();

  let container: HTMLDivElement;
  let pane: PerenePane | undefined;
  let lastBellAt = 0;

  const data = $derived(app.findPane(paneId));
  const isFiles = $derived(data?.kind === "files");
  const isAcp = $derived(data?.kind === "acp");
  /** Só o pane de terminal tem xterm; os outros desenham o próprio conteúdo. */
  const isTerm = $derived(!isFiles && !isAcp);
  const prof = $derived(profile(data?.toolProfileId ?? "shell"));
  const isActive = $derived(app.activePaneId === paneId);
  const dirLabel = $derived(baseName(data?.workingDirectory ?? "") || "~");

  /** claude/codex/opencode tocam o bell quando terminam ou esperam input (bell
   *  ligado por `cli_notify.rs`). Ignora se a janela já está com foco NESTE
   *  pane — o usuário já está olhando, notificar seria só ruído. */
  async function handleBell(): Promise<void> {
    const now = Date.now();
    if (now - lastBellAt < BELL_DEBOUNCE_MS) return;
    lastBellAt = now;
    if (document.hasFocus() && isActive) return;
    try {
      if (!(await isPermissionGranted())) return;
      sendNotification({
        title: app.findTabForPane(paneId)?.title || prof.label,
        body: t("notification.idleBody"),
      });
    } catch {
      // notificação é um extra — nunca pode derrubar o terminal.
    }
  }

  onMount(() => {
    const p = app.findPane(paneId);
    if (!p || p.kind !== "terminal") return; // só o terminal abre PTY
    pane = new PerenePane(paneId, app.settings.fontSize);
    pane
      .open(container, {
        cwd: p.workingDirectory,
        command: app.commandFor(p),
        fontSize: app.settings.fontSize,
        webgl: app.settings.webgl,
        shell: app.settings.shell || null,
        onBell: () => void handleBell(),
      })
      .catch(() => {}); // erros de spawn não devem virar unhandledrejection
  });
  onDestroy(() => pane?.dispose());

  $effect(() => {
    if (isActive && isTerm) pane?.focus();
  });

  function focusPane() {
    app.setActivePane(paneId);
    if (isTerm) pane?.focus();
  }
</script>

<div class="pane" class:active={isActive} onpointerdown={focusPane}>
  <div class="pane-head" style="--accent:{isFiles ? 'var(--accent)' : prof.color}">
    <span class="hicon" style="color:{isFiles ? 'var(--accent)' : prof.color}">
      {#if isFiles}<Code2 size={13} />{:else}<ToolIcon id={data?.toolProfileId ?? "shell"} size={13} />{/if}
    </span>
    <span class="label">{isFiles ? t("pane.editor") : prof.label}</span>
    {#if isAcp}<span class="badge">{t("acp.badge")}</span>{/if}
    <span class="dir">{dirLabel}</span>
    <span class="pstatus" style="color:{prof.color}"><StatusDot state={app.paneStatus[paneId]} /></span>
    <button class="x" title={t("pane.close") + " (⌘W)"} onclick={() => app.confirmClosePane(paneId)}><X size={13} /></button>
  </div>
  {#if isFiles}
    <div class="term"><FilesPane {paneId} /></div>
  {:else if isAcp}
    <div class="chat"><AcpPane {paneId} /></div>
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
    background: var(--bg);
    border: 1px solid transparent;
    box-sizing: border-box;
    overflow: hidden;
  }
  .pane.active {
    border-color: var(--elevated);
  }
  .pane-head {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    padding: 0 8px;
    font-size: 11px;
    color: var(--muted);
    background: var(--panel);
    border-bottom: 1px solid var(--elevated);
    flex: 0 0 auto;
    user-select: none;
  }
  .hicon {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
  }
  .label {
    color: var(--fg);
    font-weight: 600;
  }
  .dir {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pstatus {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
  }
  .x {
    display: flex;
    align-items: center;
    margin-left: auto;
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
  }
  .x:hover {
    color: var(--fg);
    background: var(--elevated);
  }
  .term {
    flex: 1 1 auto;
    min-height: 0;
    padding: 4px 6px;
    box-sizing: border-box;
  }
  /* O chat gerencia o próprio espaçamento (o composer encosta na borda). */
  .chat {
    flex: 1 1 auto;
    min-height: 0;
  }
  .badge {
    flex: 0 0 auto;
    font-size: 9px;
    letter-spacing: 0.05em;
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 0 4px;
    line-height: 13px;
  }
</style>
