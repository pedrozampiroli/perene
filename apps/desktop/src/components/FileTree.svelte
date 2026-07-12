<script lang="ts">
  import { ChevronRight, ChevronDown, Folder, FolderOpen, File } from "@lucide/svelte";
  import Self from "./FileTree.svelte";
  import { api } from "../lib/api";
  import type { DirEntry } from "../lib/types";

  let {
    entry,
    statusMap,
    onOpen,
    selected,
    depth = 0,
  }: {
    entry: DirEntry;
    statusMap: Record<string, string>;
    onOpen: (path: string) => void;
    selected: string | null;
    depth?: number;
  } = $props();

  let expanded = $state(false);
  let children = $state<DirEntry[]>([]);
  let loaded = false;

  async function activate() {
    if (entry.isDir) {
      expanded = !expanded;
      if (expanded && !loaded) {
        try {
          children = await api.fsListDir(entry.path);
        } catch {
          children = [];
        }
        loaded = true;
      }
    } else {
      onOpen(entry.path);
    }
  }

  function color(path: string): string {
    const st = statusMap[path];
    if (!st) return "";
    if (st.includes("?") || st.includes("A")) return "#4ec9b0"; // novo
    if (st.includes("M")) return "#e2c08d"; // modificado
    if (st.includes("D")) return "#f14c4c"; // removido
    return "";
  }
</script>

<div
  class="node"
  class:sel={selected === entry.path}
  style="padding-left:{depth * 12 + 8}px"
  onclick={activate}
  role="button"
  tabindex="0"
>
  <span class="caret">
    {#if entry.isDir}
      {#if expanded}<ChevronDown size={13} />{:else}<ChevronRight size={13} />{/if}
    {/if}
  </span>
  <span class="ficon">
    {#if entry.isDir}
      {#if expanded}<FolderOpen size={14} />{:else}<Folder size={14} />{/if}
    {:else}
      <File size={14} />
    {/if}
  </span>
  <span class="name" style={color(entry.path) ? `color:${color(entry.path)}` : ""}>{entry.name}</span>
</div>
{#if expanded}
  {#each children as child (child.path)}
    <Self entry={child} {statusMap} {onOpen} {selected} depth={depth + 1} />
  {/each}
{/if}

<style>
  .node {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    font-size: 12.5px;
    cursor: pointer;
    white-space: nowrap;
    color: #cccccc;
  }
  .node:hover {
    background: #2a2d2e;
  }
  .node.sel {
    background: #37373d;
  }
  .caret {
    display: flex;
    align-items: center;
    width: 13px;
    color: #8a8a8a;
    flex: 0 0 auto;
  }
  .ficon {
    display: flex;
    align-items: center;
    color: #8a99b8;
    flex: 0 0 auto;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
