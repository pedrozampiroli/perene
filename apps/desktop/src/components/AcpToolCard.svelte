<script lang="ts">
  // Cartão de uma ferramenta que o agente usou.
  //
  // A graça do ACP é justamente esta: dá para *mostrar* o que a ferramenta fez,
  // porque o conteúdo vem estruturado. No terminal, tudo isso é texto rolando.
  //
  // Colapsado por padrão quando terminou bem — o que interessa numa conversa
  // longa é a resposta, não o passo a passo. Erro e execução em andamento abrem
  // sozinhos, que é quando o detalhe importa.

  import { ChevronRight, FileText, Globe, Pencil, Search, Terminal, Wrench } from "@lucide/svelte";
  import type { AcpBlock, AcpTerminal } from "../lib/acp.svelte";
  import { t } from "../lib/i18n.svelte";
  import { baseName } from "../lib/paths";
  import AcpDiff from "./AcpDiff.svelte";

  let {
    block,
    terminals,
    onOpenFile,
  }: {
    block: AcpBlock;
    terminals: Record<string, AcpTerminal>;
    onOpenFile?: (path: string, line?: number) => void;
  } = $props();

  const failed = $derived(block.status === "failed");
  const running = $derived(block.status !== "completed" && block.status !== "failed");
  const hasBody = $derived((block.parts?.length ?? 0) > 0);

  // Abre sozinho enquanto roda ou se deu erro; fecha quando conclui bem.
  let manual = $state<boolean | null>(null);
  const open = $derived(manual ?? (running || failed));

  /** Saída do comando (vem do daemon, não do agente). */
  function outputOf(terminalId: string): AcpTerminal | undefined {
    return terminals[terminalId];
  }
</script>

<div class="card" class:done={block.status === "completed"} class:failed>
  <button
    class="head"
    class:clickable={hasBody}
    onclick={() => hasBody && (manual = !open)}
    disabled={!hasBody}
  >
    <span class="chev" class:open class:hidden={!hasBody}><ChevronRight size={11} /></span>
    <span class="icon">
      {#if block.toolKind === "execute"}<Terminal size={12} />
      {:else if block.toolKind === "edit"}<Pencil size={12} />
      {:else if block.toolKind === "read"}<FileText size={12} />
      {:else if block.toolKind === "search"}<Search size={12} />
      {:else if block.toolKind === "fetch"}<Globe size={12} />
      {:else}<Wrench size={12} />{/if}
    </span>
    <span class="title">{block.text}</span>
    {#if running}<span class="spin"></span>{/if}
  </button>

  {#if block.locations && block.locations.length > 0}
    <div class="locs">
      {#each block.locations as loc (loc.path)}
        <button class="loc" onclick={() => onOpenFile?.(loc.path, loc.line ?? undefined)}>
          {baseName(loc.path)}{loc.line ? `:${loc.line}` : ""}
        </button>
      {/each}
    </div>
  {/if}

  {#if open && block.parts}
    <div class="body">
      {#each block.parts as part, i (i)}
        {#if part.type === "text"}
          <pre class="txt">{part.text}</pre>
        {:else if part.type === "diff"}
          <AcpDiff path={part.path} oldText={part.oldText} newText={part.newText} />
        {:else}
          {@const term = outputOf(part.terminalId)}
          {#if term}
            <pre class="out">{term.output || t("acp.noOutput")}</pre>
            <div class="outfoot">
              {#if term.truncated}<span class="trunc">{t("acp.truncated")}</span>{/if}
              {#if term.exitCode !== null}
                <span class="exit" class:bad={term.exitCode !== 0}>
                  {t("acp.exitCode", { code: String(term.exitCode) })}
                </span>
              {/if}
            </div>
          {:else}
            <pre class="out dim">{t("acp.waitingOutput")}</pre>
          {/if}
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .card {
    background: #232323;
    border: 1px solid #2c2c2c;
    border-radius: 5px;
    overflow: hidden;
  }
  .card.done {
    border-color: #2f3b32;
  }
  .card.failed {
    border-color: #3f2e2e;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    background: none;
    border: none;
    color: #9aa0a6;
    font: inherit;
    font-size: 11.5px;
    padding: 5px 8px;
    text-align: left;
    cursor: default;
  }
  .head.clickable {
    cursor: pointer;
  }
  .head.clickable:hover {
    background: #2a2a2a;
  }
  .card.done .head {
    color: #7fb98a;
  }
  .card.failed .head {
    color: #e08b8b;
  }
  .chev {
    display: flex;
    flex: 0 0 auto;
    transition: transform 0.12s ease;
    color: #6a6a6a;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .chev.hidden {
    visibility: hidden;
  }
  .icon {
    display: flex;
    flex: 0 0 auto;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  /* Anel girando: mesma linguagem do indicador de status da aba. */
  .spin {
    margin-left: auto;
    flex: 0 0 auto;
    width: 9px;
    height: 9px;
    border: 1.5px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    opacity: 0.7;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin {
      animation: none;
    }
  }
  .locs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 0 8px 5px 25px;
  }
  .loc {
    background: #2a2a2a;
    border: 1px solid #333;
    border-radius: 3px;
    color: #8fb6e8;
    font-size: 10.5px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    padding: 1px 5px;
    cursor: pointer;
  }
  .loc:hover {
    background: #333;
    color: #b9d4f5;
  }
  .body {
    border-top: 1px solid #2c2c2c;
  }
  .txt,
  .out {
    margin: 0;
    padding: 6px 9px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    line-height: 1.5;
    color: #c8c8c8;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 260px;
    overflow: auto;
  }
  .out {
    background: #171717;
    color: #b8b8b8;
  }
  .out.dim {
    color: #6a6a6a;
    font-style: italic;
  }
  .outfoot {
    display: flex;
    gap: 8px;
    padding: 0 9px 5px;
    font-size: 10px;
    color: #6a6a6a;
    background: #171717;
  }
  .exit.bad {
    color: #e08b8b;
  }
  .trunc {
    font-style: italic;
  }
</style>
