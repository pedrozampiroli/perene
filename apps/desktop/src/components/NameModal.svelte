<script lang="ts">
  import { FolderOpen } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      app.confirmNameModal();
    } else if (e.key === "Escape") {
      app.nameModal = null;
    }
  }
</script>

{#if app.nameModal}
  {@const m = app.nameModal}
  <div class="backdrop" onclick={() => (app.nameModal = null)} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <h3>{m.title}</h3>

      <label class="field">
        <span>Nome</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={m.name} onkeydown={onKey} autofocus placeholder="nome…" />
      </label>

      {#if m.showDirectory}
        <label class="field">
          <span>Pasta do projeto</span>
          <div class="dir">
            <input bind:value={m.directory} onkeydown={onKey} placeholder="/caminho/do/projeto" />
            <button type="button" class="pick" onclick={() => app.pickModalDirectory()}>
              <FolderOpen size={15} /> Escolher…
            </button>
          </div>
        </label>
      {/if}

      <div class="actions">
        <button class="cancel" onclick={() => (app.nameModal = null)}>Cancelar</button>
        <button class="ok" onclick={() => app.confirmNameModal()}>Confirmar</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .modal {
    width: 420px;
    max-width: 92vw;
    background: #252526;
    color: #d4d4d4;
    border: 1px solid #3a3a3a;
    border-radius: 8px;
    padding: 18px 20px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  }
  h3 {
    margin: 0 0 14px;
    font-size: 15px;
  }
  .field {
    display: block;
    margin-bottom: 12px;
  }
  .field span {
    display: block;
    font-size: 11px;
    color: #8a8a8a;
    margin-bottom: 4px;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    background: #1e1e1e;
    border: 1px solid #3a3a3a;
    color: #fff;
    padding: 7px 10px;
    border-radius: 6px;
    outline: none;
    font-size: 13px;
  }
  input:focus {
    border-color: #007acc;
  }
  .dir {
    display: flex;
    gap: 6px;
  }
  .pick {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
    background: #333;
    border: none;
    color: #ccc;
    padding: 0 10px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
  }
  .pick:hover {
    background: #3f3f46;
    color: #fff;
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
</style>
