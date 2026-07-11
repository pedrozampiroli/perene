<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { PerenePane } from "./lib/terminal";

  let container: HTMLDivElement;
  let pane: PerenePane | undefined;

  onMount(() => {
    // M0: um único terminal local ocupando a janela.
    pane = new PerenePane("pane_main");
    void pane.open(container);
  });

  onDestroy(() => {
    pane?.dispose();
  });
</script>

<main>
  <div class="pane" bind:this={container}></div>
</main>

<style>
  main {
    position: fixed;
    inset: 0;
    background: #1e1e1e;
  }
  .pane {
    position: absolute;
    inset: 0;
    padding: 6px;
    box-sizing: border-box;
  }
</style>
