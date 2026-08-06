<script lang="ts">
  // Pane em modo ACP: a mesma CLI, só que conversando por JSON-RPC.
  //
  // Nada de estado próprio aqui — a conversa vive no daemon. Ao montar, zeramos
  // o que está na tela e pedimos o attach; o daemon reenvia o transcript inteiro
  // e a conversa reaparece exatamente como estava, mesmo que a janela tenha sido
  // fechada no meio de um turno.

  import { onMount, tick } from "svelte";
  import { ArrowUp, Check, Square, Wrench, X } from "@lucide/svelte";
  import { acp } from "../lib/acp.svelte";
  import { api } from "../lib/api";
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { acpConfig } from "../lib/profiles";

  let { paneId }: { paneId: string } = $props();

  let scroller: HTMLDivElement;
  let input = $state("");
  let sending = $state(false);
  /** Só rola sozinho se o usuário já estava no fim (não sequestra a leitura). */
  let pinned = true;

  const conv = $derived(acp.get(paneId));
  const pane = $derived(app.findPane(paneId));
  const canSend = $derived(conv.ready && !conv.busy && input.trim().length > 0);

  onMount(() => {
    const p = app.findPane(paneId);
    const cfg = p ? acpConfig(p.toolProfileId) : null;
    if (!p || !cfg) return;
    acp.reset(paneId);
    // Quem roda os comandos somos nós, presos ao diretório da sessão. Desligar
    // isto nas configurações significa que o agente sequer pode pedir.
    api
      .acpSpawn(paneId, p.workingDirectory, cfg.program, cfg.args, app.settings.acpTerminal)
      .catch(() => {}); // o erro real chega como evento `failed`, com mensagem
  });

  // Auto-scroll ao chegar conteúdo novo.
  $effect(() => {
    conv.blocks.length;
    conv.blocks[conv.blocks.length - 1]?.text;
    conv.permission;
    if (!pinned) return;
    void tick().then(() => scroller?.scrollTo({ top: scroller.scrollHeight }));
  });

  function onScroll() {
    if (!scroller) return;
    pinned = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40;
  }

  async function send() {
    const text = input.trim();
    if (!text || !conv.ready || conv.busy) return;
    sending = true;
    acp.pushUserPrompt(paneId, text);
    input = "";
    pinned = true;
    try {
      await api.acpPrompt(paneId, text);
    } finally {
      sending = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    // Enter envia, Shift+Enter quebra linha — convenção de chat.
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  }

  function answer(optionId: string | null) {
    const pending = conv.permission;
    if (!pending) return;
    acp.clearPermission(paneId);
    void api.acpPermission(paneId, pending.requestId, optionId).catch(() => {});
  }

  /** Botão "permitir" ganha destaque; o resto fica neutro. */
  function isAllow(kind?: string | null): boolean {
    return !!kind && kind.startsWith("allow");
  }
</script>

<div class="acp" onpointerdown={() => app.setActivePane(paneId)}>
  <div class="log" bind:this={scroller} onscroll={onScroll}>
    {#if !conv.ready && conv.blocks.length === 0}
      <div class="hint">{t("acp.starting")}</div>
    {/if}

    {#each conv.blocks as block (block.id)}
      {#if block.kind === "message"}
        <div class="msg" class:user={block.role === "user"}>
          <span class="who">{block.role === "user" ? t("acp.you") : t("acp.agent")}</span>
          <div class="text">{block.text}</div>
        </div>
      {:else if block.kind === "thought"}
        <div class="thought">{block.text}</div>
      {:else if block.kind === "tool"}
        <div class="tool" class:done={block.status === "completed"} class:failed={block.status === "failed"}>
          <Wrench size={12} />
          <span class="tname">{block.text}</span>
          <span class="tstatus">{block.status ?? ""}</span>
        </div>
      {:else}
        <div class="notice" class:err={block.level === "error"}>{block.text}</div>
      {/if}
    {/each}

    {#if conv.plan.length > 0}
      <ul class="plan">
        {#each conv.plan as step, i (i)}
          <li class:done={step.status === "completed"}>{step.text}</li>
        {/each}
      </ul>
    {/if}

    {#if conv.permission}
      <div class="perm">
        <div class="perm-head">{t("acp.permissionTitle")}</div>
        <div class="perm-what">{conv.permission.title}</div>
        <div class="perm-actions">
          {#each conv.permission.options as opt (opt.optionId)}
            <button class="opt" class:primary={isAllow(opt.kind)} onclick={() => answer(opt.optionId)}>
              {#if isAllow(opt.kind)}<Check size={12} />{:else}<X size={12} />{/if}
              {opt.name}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="composer">
    <textarea
      rows="1"
      bind:value={input}
      onkeydown={onKey}
      placeholder={conv.ready ? t("acp.placeholder") : t("acp.starting")}
      disabled={!conv.ready}
    ></textarea>
    {#if conv.busy}
      <button class="go stop" title={t("acp.stop")} onclick={() => api.acpCancel(paneId)}>
        <Square size={12} />
      </button>
    {:else}
      <button class="go" title={t("acp.send")} disabled={!canSend || sending} onclick={send}>
        <ArrowUp size={14} />
      </button>
    {/if}
  </div>
  <div class="foot">{pane?.workingDirectory ?? ""}</div>
</div>

<style>
  .acp {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: #1e1e1e;
    color: #d4d4d4;
    font-size: 12.5px;
    line-height: 1.55;
  }
  .log {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    padding: 10px 12px 4px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .hint {
    color: #6a6a6a;
    font-style: italic;
  }
  .msg .who {
    display: block;
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: #6a6a6a;
    margin-bottom: 2px;
  }
  .msg .text {
    white-space: pre-wrap;
    word-break: break-word;
  }
  .msg.user .text {
    color: #cfd6dd;
    border-left: 2px solid #3a3d41;
    padding-left: 8px;
  }
  .thought {
    color: #7b7b7b;
    font-style: italic;
    white-space: pre-wrap;
    border-left: 2px solid #2f2f2f;
    padding-left: 8px;
  }
  .tool {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #9aa0a6;
    background: #232323;
    border: 1px solid #2c2c2c;
    border-radius: 5px;
    padding: 4px 8px;
    font-size: 11.5px;
  }
  .tool.done {
    color: #7fb98a;
    border-color: #2f3b32;
  }
  .tool.failed {
    color: #e08b8b;
    border-color: #3f2e2e;
  }
  .tname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tstatus {
    margin-left: auto;
    color: #6a6a6a;
    font-size: 10px;
  }
  .plan {
    margin: 0;
    padding-left: 18px;
    color: #9aa0a6;
    font-size: 11.5px;
  }
  .plan li.done {
    color: #6a6a6a;
    text-decoration: line-through;
  }
  .notice {
    color: #9aa0a6;
    font-size: 11.5px;
  }
  /* Falha de adapter vem com o stderr junto: várias linhas, e é justamente
     isso que diz o motivo. Preserva as quebras, mas sem tomar a tela. */
  .notice.err {
    color: #e08b8b;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    max-height: 220px;
    overflow-y: auto;
    background: #2a1e1e;
    border: 1px solid #3f2e2e;
    border-radius: 5px;
    padding: 6px 8px;
  }
  .perm {
    border: 1px solid #4a3f22;
    background: #2a2418;
    border-radius: 6px;
    padding: 8px 10px;
  }
  .perm-head {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #d9b45f;
    margin-bottom: 3px;
  }
  .perm-what {
    color: #e3e3e3;
    word-break: break-word;
    margin-bottom: 8px;
  }
  .perm-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 5px;
    background: #2f2f2f;
    border: 1px solid #3a3d41;
    color: #cccccc;
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 11.5px;
    cursor: pointer;
  }
  .opt:hover {
    background: #3a3d41;
  }
  .opt.primary {
    background: #2f4630;
    border-color: #3d5c3f;
    color: #cfe8d1;
  }
  .opt.primary:hover {
    background: #3a5a3c;
  }
  .composer {
    display: flex;
    align-items: flex-end;
    gap: 6px;
    padding: 8px 12px;
    border-top: 1px solid #2a2a2a;
    flex: 0 0 auto;
  }
  textarea {
    flex: 1 1 auto;
    resize: none;
    max-height: 140px;
    min-height: 26px;
    background: #252526;
    border: 1px solid #3a3d41;
    border-radius: 5px;
    color: #d4d4d4;
    padding: 5px 8px;
    font: inherit;
    font-family: inherit;
    line-height: 1.5;
    field-sizing: content;
  }
  textarea:focus {
    outline: none;
    border-color: #4a7fb5;
  }
  textarea:disabled {
    color: #6a6a6a;
  }
  .go {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    flex: 0 0 auto;
    background: #4a7fb5;
    border: none;
    border-radius: 5px;
    color: #fff;
    cursor: pointer;
  }
  .go:disabled {
    background: #2f2f2f;
    color: #6a6a6a;
    cursor: default;
  }
  .go.stop {
    background: #6b4a4a;
  }
  .foot {
    flex: 0 0 auto;
    padding: 0 12px 6px;
    font-size: 10px;
    color: #5a5a5a;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
