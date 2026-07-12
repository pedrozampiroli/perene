<script lang="ts">
  import { onMount } from "svelte";
  import {
    GitBranch,
    RefreshCw,
    ArrowDownToLine,
    ArrowUpToLine,
    GitPullRequestArrow,
    FolderTree,
    Check,
  } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import type { GitStatus } from "../lib/types";

  let gs = $state<GitStatus | null>(null);
  let menu = $state(false);
  let branches = $state<string[]>([]);
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
    if (menu && gs?.root) {
      try {
        branches = await api.gitBranches(gs.root);
      } catch {
        branches = [];
      }
    }
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
    <button class="branch" onclick={toggleMenu} title={"Branch: " + gs.branch}>
      <GitBranch size={13} />
      <span class="bn">{gs.branch}</span>
      {#if gs.dirty}<span class="dot" title="mudanças não commitadas"></span>{/if}
      {#if gs.ahead}<span class="ab">↑{gs.ahead}</span>{/if}
      {#if gs.behind}<span class="ab">↓{gs.behind}</span>{/if}
    </button>

    {#if menu}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="backdrop" onclick={() => (menu = false)}></div>
      <div class="menu">
        <button onclick={() => act((r) => api.gitFetch(r), "fetch ok")}><RefreshCw size={13} /> Fetch</button>
        <button onclick={() => act((r) => api.gitPull(r), "pull ok")}><ArrowDownToLine size={13} /> Pull</button>
        <button onclick={() => act((r) => api.gitPush(r), "push ok")}><ArrowUpToLine size={13} /> Push</button>
        <button onclick={() => act((r) => api.gitOpenPr(r), "PR aberto")}><GitPullRequestArrow size={13} /> Abrir PR</button>
        <button onclick={() => { app.openFilesTab(); menu = false; }}><FolderTree size={13} /> Painel de arquivos</button>
        <div class="sep"></div>
        <div class="mlabel">Branches</div>
        <div class="blist">
          {#each branches as b (b)}
            <button class="bitem" onclick={() => act((r) => api.gitCheckout(r, b), "→ " + b)}>
              {#if b === gs.branch}<Check size={13} />{:else}<span class="sp"></span>{/if}
              {b}
            </button>
          {/each}
        </div>
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
    color: #d4a72c;
    cursor: pointer;
    font-size: 12px;
    padding: 3px 8px;
    border-radius: 5px;
    overflow: hidden;
  }
  .branch:hover {
    background: #ffffff12;
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
    background: #e2c08d;
    flex: 0 0 auto;
  }
  .ab {
    color: #9aa0a6;
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
    background: #252526;
    border: 1px solid #3a3a3a;
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
    color: #cccccc;
    cursor: pointer;
    padding: 6px 10px;
    border-radius: 5px;
    font-size: 12.5px;
    text-align: left;
  }
  .menu > button:hover {
    background: #2a2d2e;
  }
  .sep {
    height: 1px;
    background: #3a3a3a;
    margin: 4px 2px;
  }
  .mlabel {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #7a7a7a;
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
    color: #cccccc;
    cursor: pointer;
    padding: 5px 10px;
    border-radius: 5px;
    font-size: 12.5px;
    text-align: left;
  }
  .bitem:hover {
    background: #2a2d2e;
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
    background: #2d2d30;
    border: 1px solid #3a3a3a;
    border-left: 3px solid #d4a72c;
    color: #d4d4d4;
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 12px;
    cursor: pointer;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
    word-break: break-word;
  }
</style>
