<script lang="ts">
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
        <button class="cancel" onclick={() => (app.confirm = null)}>Cancelar</button>
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
    background: #252526;
    color: #d4d4d4;
    border: 1px solid #3a3a3a;
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
    color: #e2c08d;
  }
  .warn.danger {
    color: #f14c4c;
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0 0 4px;
    font-size: 13px;
    color: #c9c9c9;
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
    background: #3a3d41;
    color: #ddd;
  }
  .cancel:hover {
    background: #4a4d51;
  }
  .ok {
    background: #0e639c;
    color: #fff;
  }
  .ok:hover {
    background: #1177bb;
  }
  .ok.danger {
    background: #c0392b;
  }
  .ok.danger:hover {
    background: #e04434;
  }
</style>
