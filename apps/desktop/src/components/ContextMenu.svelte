<script lang="ts">
  import { app } from "../lib/store.svelte";

  let el = $state<HTMLDivElement>();

  // Mantém o menu dentro da janela.
  $effect(() => {
    const m = app.contextMenu;
    if (!m || !el) return;
    const r = el.getBoundingClientRect();
    let x = m.x;
    let y = m.y;
    if (x + r.width > window.innerWidth - 8) x = window.innerWidth - r.width - 8;
    if (y + r.height > window.innerHeight - 8) y = window.innerHeight - r.height - 8;
    el.style.left = `${Math.max(4, x)}px`;
    el.style.top = `${Math.max(4, y)}px`;
  });

  function run(item: { action?: () => void; disabled?: boolean }) {
    if (item.disabled) return;
    app.contextMenu = null;
    item.action?.();
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") app.contextMenu = null;
  }}
/>

{#if app.contextMenu}
  {@const m = app.contextMenu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="backdrop"
    onpointerdown={() => (app.contextMenu = null)}
    oncontextmenu={(e) => {
      e.preventDefault();
      app.contextMenu = null;
    }}
  ></div>
  <div class="menu" bind:this={el} style="left:{m.x}px; top:{m.y}px" role="menu" tabindex="-1">
    {#each m.items as item, i (i)}
      {#if item.separator}
        <div class="sep"></div>
      {:else}
        <button class="item" class:danger={item.danger} disabled={item.disabled} onclick={() => run(item)} role="menuitem">
          {item.label}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 400;
  }
  .menu {
    position: fixed;
    z-index: 401;
    min-width: 200px;
    max-height: 70vh;
    overflow-y: auto;
    background: #252526;
    border: 1px solid #3a3a3a;
    border-radius: 8px;
    padding: 4px;
    box-shadow: 0 10px 32px rgba(0, 0, 0, 0.6);
  }
  .item {
    display: block;
    width: 100%;
    background: none;
    border: none;
    color: #cccccc;
    text-align: left;
    padding: 6px 10px;
    border-radius: 5px;
    font-size: 12.5px;
    cursor: pointer;
    white-space: nowrap;
  }
  .item:hover:not(:disabled) {
    background: #04395e;
    color: #fff;
  }
  .item:disabled {
    color: #5a5a5a;
    cursor: default;
  }
  .item.danger {
    color: #f14c4c;
  }
  .item.danger:hover:not(:disabled) {
    background: #5a1d1d;
    color: #ff8b8b;
  }
  .sep {
    height: 1px;
    background: #3a3a3a;
    margin: 4px 2px;
  }
</style>
