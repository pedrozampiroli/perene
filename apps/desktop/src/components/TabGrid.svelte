<script lang="ts">
  import SplitContainer from "./SplitContainer.svelte";
  import { app } from "../lib/store.svelte";

  const tab = $derived(app.activeTab);
</script>

{#if tab}
  <!-- key na aba: trocar de aba desmonta os terminais da anterior (restore lazy:
       só a aba ativa atacha PTYs; o daemon preserva os demais). -->
  {#key tab.id}
    <div class="tabgrid">
      <SplitContainer node={tab.layout} />
    </div>
  {/key}
{:else}
  <div class="empty">
    <p>Nenhuma aba aberta.</p>
    <button onclick={() => app.createTab("shell")}>+ Novo terminal</button>
  </div>
{/if}

<style>
  .tabgrid {
    height: 100%;
    width: 100%;
  }
  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: #6a6a6a;
  }
  .empty button {
    background: #0e639c;
    color: white;
    border: none;
    padding: 8px 16px;
    border-radius: 4px;
    cursor: pointer;
  }
</style>
