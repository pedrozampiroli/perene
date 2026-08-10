<script lang="ts">
  // Diff de uma edição que o agente propôs/fez.
  //
  // Diff próprio (LCS por linha) em vez do CodeMirror merge do editor: aqui são
  // trechos curtos dentro de um cartão de chat, muitos por conversa. Montar um
  // editor completo para cada um custaria caro em RAM — e o alvo do projeto é
  // < 150 MB.

  import { FilePlus2, FileDiff } from "@lucide/svelte";
  import { buildDiff, diffStats } from "../lib/diff";
  import { baseName } from "../lib/paths";

  let {
    path,
    oldText,
    newText,
  }: { path: string; oldText: string | null; newText: string } = $props();

  /** `null` em oldText = arquivo novo (o agente está criando). */
  const isNew = $derived(oldText === null);

  const lines = $derived(buildDiff(oldText, newText));
  const stats = $derived(diffStats(lines));
</script>

<div class="diff">
  <div class="head">
    {#if isNew}<FilePlus2 size={11} />{:else}<FileDiff size={11} />{/if}
    <span class="path" title={path}>{baseName(path) || path}</span>
    <span class="stats">
      {#if stats.add}<span class="plus">+{stats.add}</span>{/if}
      {#if stats.del}<span class="minus">−{stats.del}</span>{/if}
    </span>
  </div>
  <div class="body">
    {#each lines as line, i (i)}
      {#if line.kind === "gap"}
        <div class="row gap">{line.text}</div>
      {:else}
        <div class="row {line.kind}">
          <span class="sign">{line.kind === "add" ? "+" : line.kind === "del" ? "−" : " "}</span
          ><span class="txt">{line.text}</span>
        </div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .diff {
    background: #171717;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    line-height: 1.5;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    color: #9aa0a6;
    background: #1d1d1d;
    border-bottom: 1px solid #2c2c2c;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stats {
    margin-left: auto;
    display: flex;
    gap: 6px;
    flex: 0 0 auto;
  }
  .plus {
    color: #7fb98a;
  }
  .minus {
    color: #e08b8b;
  }
  .body {
    max-height: 300px;
    overflow: auto;
  }
  .row {
    display: flex;
    white-space: pre-wrap;
    word-break: break-word;
    padding-right: 8px;
  }
  .sign {
    flex: 0 0 auto;
    width: 16px;
    text-align: center;
    color: #6a6a6a;
    user-select: none;
  }
  .txt {
    flex: 1 1 auto;
  }
  .row.keep {
    color: #8a8a8a;
  }
  .row.add {
    background: #172a1c;
    color: #a8d8b0;
  }
  .row.add .sign {
    color: #7fb98a;
  }
  .row.del {
    background: #2a1a1a;
    color: #e0aaaa;
  }
  .row.del .sign {
    color: #e08b8b;
  }
  .row.gap {
    color: #5a5a5a;
    font-style: italic;
    padding-left: 16px;
    background: #1a1a1a;
  }
</style>
