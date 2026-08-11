<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { onMount } from "svelte";
  import {
    GitBranch,
    RefreshCw,
    ArrowDownToLine,
    ArrowUpToLine,
    GitPullRequestArrow,
    FolderTree,
    Check,
    ChevronRight,
    Plus,
  } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import type { GitStatus } from "../lib/types";

  let gs = $state<GitStatus | null>(null);
  let menu = $state(false);
  let showBranches = $state(false);
  let branches = $state<string[]>([]);
  let newBranch = $state("");
  let msg = $state("");
  let msgTimer: ReturnType<typeof setTimeout>;

  // cwd do pane ativo (ou, na falta, do workspace).
  const cwd = $derived(
    app.findPane(app.activePaneId ?? "")?.workingDirectory ??
      app.activeTab?.panes[0]?.workingDirectory ??
      app.activeWorkspace?.directory ??
      app.home,
  );

  async function load() {
    if (!cwd) {
      gs = null;
      return;
    }
    try {
      gs = await api.gitStatus(cwd);
    } catch {
      gs = null;
    }
  }

  // Recarrega quando o pane ativo (cwd) muda.
  $effect(() => {
    void cwd;
    load();
  });

  // Poll leve para refletir mudanças externas.
  onMount(() => {
    const t = setInterval(load, 6000);
    return () => clearInterval(t);
  });

  async function toggleMenu() {
    menu = !menu;
    showBranches = false;
    if (menu && gs?.root) {
      try {
        branches = await api.gitBranches(gs.root);
      } catch {
        branches = [];
      }
    }
  }

  async function createBranch() {
    const b = newBranch.trim();
    if (!b) return;
    newBranch = "";
    await act((r) => api.gitCreateBranch(r, b), t("git.branchCreated", { branch: b }));
  }

  function flash(t: string) {
    msg = t;
    clearTimeout(msgTimer);
    msgTimer = setTimeout(() => (msg = ""), t.length > 40 ? 7000 : 3000);
  }

  async function act(fn: (r: string) => Promise<unknown>, ok: string) {
    if (!gs?.root) return;
    menu = false;
    try {
      await fn(gs.root);
      flash(ok);
      await load();
    } catch (e) {
      flash(String(e));
    }
  }
</script>

{#if gs?.isRepo}
  <div class="gitw">
    <button class="branch" onclick={toggleMenu} title={t("git.branch", { branch: gs.branch })}>
      <GitBranch size={13} />
      <span class="bn">{gs.branch}</span>
      {#if gs.dirty}<span class="dot" title={t("git.dirty")}></span>{/if}
      {#if gs.ahead}<span class="ab">↑{gs.ahead}</span>{/if}
      {#if gs.behind}<span class="ab">↓{gs.behind}</span>{/if}
    </button>

    {#if menu}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="backdrop" onclick={() => (menu = false)}></div>
      <div class="menu">
        <button class="row" onclick={() => (showBranches = !showBranches)}>
          <span class="l"><GitBranch size={13} /> {t("git.switchBranch")}</span>
          <ChevronRight size={14} style="transform: rotate({showBranches ? 90 : 0}deg)" />
        </button>
        {#if showBranches}
          <div class="blist">
            {#each branches as b (b)}
              <button class="bitem" onclick={() => act((r) => api.gitCheckout(r, b), t("git.switchedTo", { branch: b }))}>
                {#if b === gs.branch}<Check size={13} />{:else}<span class="sp"></span>{/if}
                {b}
              </button>
            {:else}
              <div class="mempty">{t("git.noBranches")}</div>
            {/each}
          </div>
        {/if}
        <div class="newb">
          <input placeholder={t("git.newBranch")} bind:value={newBranch} onkeydown={(e) => e.key === "Enter" && createBranch()} />
          <button class="plus" title={t("git.createBranch")} onclick={createBranch}><Plus size={14} /></button>
        </div>

        <div class="sep"></div>
        <button onclick={() => act((r) => api.gitFetch(r), t("git.fetchOk"))}><RefreshCw size={13} /> {t("git.fetch")}</button>
        <button onclick={() => act((r) => api.gitPull(r), t("git.pullOk"))}><ArrowDownToLine size={13} /> {t("git.pull")}</button>
        <button onclick={() => act((r) => api.gitPush(r), t("git.pushOk"))}><ArrowUpToLine size={13} /> {t("git.push")}</button>

        <div class="sep"></div>
        <button onclick={() => act((r) => api.gitOpenPr(r), t("git.prOpened"))}><GitPullRequestArrow size={13} /> {t("git.pullRequests")}</button>
        <button onclick={() => { app.openFilesTab(); menu = false; }}><FolderTree size={13} /> {t("git.openEditor")}</button>
      </div>
    {/if}
  </div>
{/if}

{#if msg}
  <button class="toast" onclick={() => (msg = "")}>{msg}</button>
{/if}

<style>
  .gitw {
    position: relative;
  }
  .branch {
    display: flex;
    align-items: center;
    gap: 5px;
    max-width: 240px;
    background: none;
    border: none;
    color: var(--warning);
    cursor: pointer;
    font-size: 12px;
    padding: 3px 8px;
    border-radius: 5px;
    overflow: hidden;
  }
  .branch:hover {
    background: color-mix(in srgb, var(--fg) 7%, transparent);
  }
  .branch :global(svg) {
    flex: 0 0 auto;
  }
  .bn {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warning);
    flex: 0 0 auto;
  }
  .ab {
    color: var(--muted);
    font-size: 11px;
    flex: 0 0 auto;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
  }
  .menu {
    position: absolute;
    top: 28px;
    right: 0;
    z-index: 41;
    min-width: 220px;
    max-height: 60vh;
    overflow-y: auto;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 4px;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.55);
  }
  .menu > button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    background: none;
    border: none;
    color: var(--fg);
    cursor: pointer;
    padding: 6px 10px;
    border-radius: 5px;
    font-size: 12.5px;
    text-align: left;
  }
  .menu > button:hover {
    background: var(--elevated);
  }
  .menu > button.row {
    justify-content: space-between;
  }
  .row .l {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .newb {
    display: flex;
    gap: 5px;
    padding: 4px 6px 2px;
  }
  .newb input {
    flex: 1 1 auto;
    min-width: 0;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--fg);
    border-radius: 5px;
    padding: 5px 8px;
    outline: none;
    font-size: 12px;
  }
  .plus {
    display: flex;
    align-items: center;
    background: var(--border);
    border: none;
    color: var(--fg);
    border-radius: 5px;
    padding: 0 8px;
    cursor: pointer;
  }
  .plus:hover {
    background: var(--border);
    color: var(--fg);
  }
  .mempty {
    padding: 4px 12px;
    font-size: 11px;
    color: var(--muted);
  }
  .sep {
    height: 1px;
    background: var(--border);
    margin: 4px 2px;
  }
  .mlabel {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    padding: 4px 10px 2px;
  }
  .blist {
    max-height: 200px;
    overflow-y: auto;
  }
  .bitem {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    background: none;
    border: none;
    color: var(--fg);
    cursor: pointer;
    padding: 5px 10px;
    border-radius: 5px;
    font-size: 12.5px;
    text-align: left;
  }
  .bitem:hover {
    background: var(--elevated);
  }
  .bitem .sp {
    width: 13px;
    flex: 0 0 auto;
  }
  .toast {
    position: fixed;
    top: 40px;
    right: 12px;
    z-index: 60;
    max-width: 460px;
    text-align: left;
    background: var(--elevated);
    border: 1px solid var(--border);
    border-left: 3px solid var(--warning);
    color: var(--fg);
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 12px;
    cursor: pointer;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
    word-break: break-word;
  }
</style>
