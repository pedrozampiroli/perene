<script lang="ts">
  import PaneView from "./PaneView.svelte";
  import Self from "./SplitContainer.svelte";
  import { app } from "../lib/store.svelte";
  import type { LayoutNode } from "../lib/types";

  let { node }: { node: LayoutNode } = $props();

  let containerEl = $state<HTMLDivElement>();
  let dragging = $state(false);

  function startDrag(e: PointerEvent, splitId: string, direction: string) {
    if (!containerEl) return;
    e.preventDefault();
    dragging = true;
    const rect = containerEl.getBoundingClientRect();
    const move = (ev: PointerEvent) => {
      const ratio =
        direction === "horizontal"
          ? (ev.clientX - rect.left) / rect.width
          : (ev.clientY - rect.top) / rect.height;
      app.setSplitRatio(splitId, ratio);
    };
    const up = () => {
      dragging = false;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
</script>

{#if node.type === "leaf"}
  <PaneView paneId={node.paneId} />
{:else}
  <div class="split {node.direction}" class:dragging bind:this={containerEl}>
    <div class="child" style="flex: {node.ratio} 1 0">
      <Self node={node.children[0]} />
    </div>
    <div
      class="divider {node.direction}"
      onpointerdown={(e) => startDrag(e, node.id, node.direction)}
    ></div>
    <div class="child" style="flex: {1 - node.ratio} 1 0">
      <Self node={node.children[1]} />
    </div>
  </div>
{/if}

<style>
  .split {
    display: flex;
    height: 100%;
    width: 100%;
  }
  .split.horizontal {
    flex-direction: row;
  }
  .split.vertical {
    flex-direction: column;
  }
  .child {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .divider {
    flex: 0 0 6px;
    background: #2a2a2a;
  }
  .divider.horizontal {
    cursor: col-resize;
  }
  .divider.vertical {
    cursor: row-resize;
  }
  .divider:hover,
  .dragging .divider {
    background: #007acc;
  }
</style>
