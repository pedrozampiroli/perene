<script lang="ts">
  import { t } from "../lib/i18n.svelte";
  import { TriangleAlert } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") app.confirm = null;
    else if (e.key === "Enter") app.runConfirm();
  }
</script>

<svelte:window onkeydown={app.confirm ? onKey : undefined} />

{#if app.confirm}
  {@const c = app.confirm}
  <div class="backdrop" onclick={() => (app.confirm = null)} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="head">
        <span class="warn" class:danger={c.danger}><TriangleAlert size={18} /></span>
        <h3>{c.title}</h3>
      </div>
      <p>{c.message}</p>
      <div class="actions">
        <button class="cancel" onclick={() => (app.confirm = null)}>{t("confirm.cancel")}</button>
        <button class="ok" class:danger={c.danger} onclick={() => app.runConfirm()}>{c.confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 300;
  }
  .modal {
    width: 400px;
    max-width: 92vw;
    background: var(--panel);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 18px 20px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.55);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .warn {
    display: flex;
    color: var(--warning);
  }
  .warn.danger {
    color: var(--danger);
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0 0 4px;
    font-size: 13px;
    color: var(--fg);
    line-height: 1.5;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
  .actions button {
    border: none;
    padding: 7px 16px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }
  .cancel {
    background: var(--elevated);
    color: var(--fg);
  }
  .cancel:hover {
    background: var(--border);
  }
  .ok {
    background: var(--accent);
    color: var(--accent-fg);
  }
  .ok:hover {
    background: var(--accent);
  }
  .ok.danger {
    background: var(--danger);
  }
  .ok.danger:hover {
    background: var(--danger);
  }
</style>
