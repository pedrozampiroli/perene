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
    background: color-mix(in srgb, var(--fg) 4%, var(--bg));
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    line-height: 1.5;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    color: var(--muted);
    background: var(--panel);
    border-bottom: 1px solid var(--border);
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
    color: var(--success);
  }
  .minus {
    color: var(--danger);
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
    color: var(--muted);
    user-select: none;
  }
  .txt {
    flex: 1 1 auto;
  }
  .row.keep {
    color: var(--muted);
  }
  .row.add {
    background: color-mix(in srgb, var(--success) 14%, var(--bg));
    color: var(--success);
  }
  .row.add .sign {
    color: var(--success);
  }
  .row.del {
    background: color-mix(in srgb, var(--danger) 14%, var(--bg));
    color: var(--danger);
  }
  .row.del .sign {
    color: var(--danger);
  }
  .row.gap {
    color: var(--muted);
    font-style: italic;
    padding-left: 16px;
    background: color-mix(in srgb, var(--fg) 3%, var(--bg));
  }
</style>
