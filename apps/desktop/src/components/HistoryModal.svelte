<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { onMount } from "svelte";
  import { X } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { profile } from "../lib/profiles";
  import { baseName } from "../lib/paths";
  import type { SessionRecord } from "../lib/types";

  let all = $state<SessionRecord[]>([]);
  let loading = $state(true);
  let query = $state("");
  let harness = $state("all");
  let selected = $state<SessionRecord | null>(null);
  let transcript = $state("");

  const HARNESSES = [
    { id: "all", label: t("history.all") },
    { id: "claude", label: "Claude" },
    { id: "codex", label: "Codex" },
    { id: "opencode", label: "OpenCode" },
  ];

  const filtered = $derived(
    all.filter((r) => {
      if (harness !== "all" && r.harness !== harness) return false;
      if (query) {
        const hay = `${r.title ?? ""} ${r.projectPath} ${r.harness}`.toLowerCase();
        if (!hay.includes(query.toLowerCase())) return false;
      }
      return true;
    }),
  );

  onMount(async () => {
    try {
      all = await api.sessionHistoryLoad();
    } finally {
      loading = false;
    }
  });

  async function select(rec: SessionRecord) {
    selected = rec;
    transcript = t("history.loadingPreview");
    try {
      transcript = (await api.sessionTranscript(rec)) || t("history.noPreview");
    } catch {
      transcript = t("history.noPreview");
    }
  }

  function fmtDate(ms: number): string {
    return new Date(ms).toLocaleString("pt-BR", { dateStyle: "medium", timeStyle: "short" });
  }
  function projectName(path: string): string {
    return baseName(path) || path;
  }
</script>

<div class="backdrop" onclick={() => (app.historyOpen = false)} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
    <div class="top">
      <input class="search" placeholder={t("history.search")} bind:value={query} />
      <div class="seg">
        {#each HARNESSES as h (h.id)}
          <button class:active={harness === h.id} onclick={() => (harness = h.id)}>{h.label}</button>
        {/each}
      </div>
      <button class="close" onclick={() => (app.historyOpen = false)}><X size={16} /></button>
    </div>

    <div class="body">
      <div class="list">
        {#if loading}
          <div class="muted">{t("history.loading")}</div>
        {:else if filtered.length === 0}
          <div class="muted">{t("history.none")}</div>
        {:else}
          {#each filtered as rec (rec.harness + rec.sessionId)}
            <div
              class="row"
              class:sel={selected?.sessionId === rec.sessionId && selected?.harness === rec.harness}
              onclick={() => select(rec)}
              ondblclick={() => app.openHistorySession(rec)}
              role="button"
              tabindex="0"
            >
              <span class="dot" style="background:{profile(rec.harness).color}"></span>
              <div class="meta">
                <div class="title">{rec.title || projectName(rec.projectPath) + " · " + rec.sessionId.slice(0, 8)}</div>
                <div class="sub">{profile(rec.harness).label} · {projectName(rec.projectPath)} · {fmtDate(rec.dateMs)}</div>
              </div>
            </div>
          {/each}
        {/if}
      </div>

      <div class="preview">
        {#if selected}
          <div class="phead">
            {profile(selected.harness).label} · {selected.projectPath}
          </div>
          <pre>{transcript}</pre>
          <button class="resume" onclick={() => app.openHistorySession(selected!)}>
            {t("history.resume")}
          </button>
        {:else}
          <div class="muted center">{t("history.selectSession")}</div>
        {/if}
      </div>
    </div>
    <div class="status">{loading ? "…" : t("history.count", { count: filtered.length })}</div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 900px;
    height: 560px;
    max-width: 92vw;
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  }
  .top {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  .search {
    flex: 1 1 auto;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--fg);
    padding: 6px 10px;
    border-radius: 6px;
    outline: none;
  }
  .seg {
    display: flex;
    gap: 2px;
    background: var(--bg);
    border-radius: 6px;
    padding: 2px;
  }
  .seg button {
    background: none;
    border: none;
    color: var(--muted);
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }
  .seg button.active {
    background: var(--elevated);
    color: var(--fg);
  }
  .close {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    font-size: 14px;
  }
  .body {
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: 1fr 1fr;
    min-height: 0;
  }
  .list {
    overflow-y: auto;
    border-right: 1px solid var(--border);
    padding: 6px;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px;
    border-radius: 6px;
    cursor: pointer;
  }
  .row:hover {
    background: var(--elevated);
  }
  .row.sel {
    background: var(--elevated);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex: 0 0 auto;
  }
  .meta {
    min-width: 0;
  }
  .title {
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .preview {
    display: flex;
    flex-direction: column;
    padding: 10px 12px;
    min-height: 0;
  }
  .phead {
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  pre {
    flex: 1 1 auto;
    overflow-y: auto;
    margin: 0;
    font-size: 11px;
    white-space: pre-wrap;
    color: var(--fg);
  }
  .resume {
    margin-top: 8px;
    align-self: flex-end;
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    padding: 7px 14px;
    border-radius: 6px;
    cursor: pointer;
  }
  .muted {
    color: var(--muted);
    padding: 12px;
    font-size: 13px;
  }
  .center {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .status {
    padding: 6px 12px;
    font-size: 11px;
    color: var(--muted);
    border-top: 1px solid var(--border);
  }
</style>
