<script lang="ts">
  // Pane em modo ACP: a mesma CLI, só que conversando por JSON-RPC.
  //
  // Nada de estado próprio aqui — a conversa vive no daemon. Ao montar, zeramos
  // o que está na tela e pedimos o attach; o daemon reenvia o transcript inteiro
  // e a conversa reaparece exatamente como estava, mesmo que a janela tenha sido
  // fechada no meio de um turno.

  import { onMount, tick } from "svelte";
  import { ArrowUp, AtSign, Brain, Check, ChevronRight, GitFork, Square, X } from "@lucide/svelte";
  import { acp } from "../lib/acp.svelte";
  import { api } from "../lib/api";
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { renderMarkdown } from "../lib/markdown";
  import { baseName } from "../lib/paths";
  import { acpConfig } from "../lib/profiles";
  import type { AcpImage, AcpMention } from "../lib/types";
  import AcpToolCard from "./AcpToolCard.svelte";

  let { paneId }: { paneId: string } = $props();

  let scroller: HTMLDivElement;
  let box: HTMLTextAreaElement;
  let input = $state("");
  let sending = $state(false);
  /** Imagens coladas, aguardando envio junto do texto. */
  let anexos = $state<AcpImage[]>([]);
  /** Arquivos mencionados com `@`, idem. */
  let mencoes = $state<AcpMention[]>([]);
  /** Raciocínios expandidos (colapsados por padrão, como no Zed). */
  let pensamentosAbertos = $state<Record<number, boolean>>({});
  /** Índice selecionado no menu de autocomplete (−1 = fechado). */
  let sugIndex = $state(-1);
  /** Arquivos do projeto, carregados sob demanda no primeiro `@`. */
  let arquivos = $state<string[]>([]);
  let carregandoArquivos = false;
  /** Só rola sozinho se o usuário já estava no fim (não sequestra a leitura). */
  let pinned = true;

  const conv = $derived(acp.get(paneId));
  const pane = $derived(app.findPane(paneId));
  const canSend = $derived(
    conv.ready && !conv.busy && (input.trim().length > 0 || anexos.length > 0 || mencoes.length > 0),
  );

  // ── Autocomplete: `/comando` e `@arquivo` ─────────────────────────────────
  // Só enquanto a palavra está sendo digitada no fim do texto; depois do espaço
  // o usuário já escolheu.
  const gatilho = $derived.by(() => {
    const m = /(^|\s)([/@])(\S*)$/.exec(input);
    if (!m) return null;
    // `/` só vale no começo do prompt (é comando, não caminho).
    if (m[2] === "/" && m.index !== 0) return null;
    return { tipo: m[2] as "/" | "@", termo: m[3].toLowerCase(), inicio: m.index + m[1].length };
  });

  const sugestoes = $derived.by(() => {
    if (!gatilho) return [] as { valor: string; rotulo: string; detalhe: string }[];
    if (gatilho.tipo === "/") {
      return conv.commands
        .filter((c) => c.name.toLowerCase().startsWith(gatilho.termo))
        .slice(0, 8)
        .map((c) => ({ valor: c.name, rotulo: `/${c.name}`, detalhe: c.description }));
    }
    const termo = gatilho.termo;
    return arquivos
      .filter((f) => f.toLowerCase().includes(termo))
      .slice(0, 8)
      .map((f) => ({ valor: f, rotulo: baseName(f), detalhe: f }));
  });

  $effect(() => {
    sugIndex = sugestoes.length > 0 ? Math.min(Math.max(sugIndex, 0), sugestoes.length - 1) : -1;
  });

  // Carrega a lista de arquivos na primeira vez que o usuário digita `@`.
  $effect(() => {
    if (gatilho?.tipo !== "@" || arquivos.length > 0 || carregandoArquivos) return;
    const dir = app.findPane(paneId)?.workingDirectory;
    if (!dir) return;
    carregandoArquivos = true;
    void api
      .fsListFiles(dir, 5000)
      .then((f) => (arquivos = f))
      .catch(() => {})
      .finally(() => (carregandoArquivos = false));
  });

  function escolher(i: number) {
    const s = sugestoes[i];
    if (!s || !gatilho) return;
    if (gatilho.tipo === "/") {
      input = `/${s.valor} `;
    } else {
      const dir = app.findPane(paneId)?.workingDirectory ?? "";
      const absoluto = s.valor.startsWith("/") ? s.valor : `${dir}/${s.valor}`;
      if (!mencoes.some((m) => m.path === absoluto)) {
        mencoes = [...mencoes, { path: absoluto, name: baseName(s.valor) }];
      }
      // Tira o `@termo` do texto: o arquivo vira um chip acima da caixa.
      input = input.slice(0, gatilho.inicio) + input.slice(gatilho.inicio).replace(/^@\S*/, "");
    }
    sugIndex = -1;
    box?.focus();
  }

  onMount(() => {
    const p = app.findPane(paneId);
    const cfg = p ? acpConfig(p.toolProfileId) : null;
    if (!p || !cfg) return;
    acp.reset(paneId);
    // Aba de fork: o `acpFork` já subiu a sessão e atachou. Spawnar aqui criaria
    // uma conversa nova por cima, jogando fora justamente o que foi bifurcado.
    if (app.takeForkPending(paneId)) return;
    // Quem roda os comandos somos nós, presos ao diretório da sessão. Desligar
    // isto nas configurações significa que o agente sequer pode pedir.
    api
      .acpSpawn(paneId, p.workingDirectory, cfg.program, cfg.args, app.settings.acpTerminal)
      .catch(() => {}); // o erro real chega como evento `failed`, com mensagem
  });

  // Guarda o id da sessão no manifest assim que ele chega.
  $effect(() => {
    if (conv.sessionId) app.rememberAcpSession(paneId, conv.sessionId);
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
    if (!canSend) return;
    const images = anexos;
    const links = mencoes;
    sending = true;
    // O eco mostra as menções junto — é o que o agente vai receber.
    const eco = links.length > 0 ? `${text}\n\n${links.map((m) => `\`@${m.name}\``).join(" ")}` : text;
    acp.pushUserPrompt(paneId, eco, images);
    input = "";
    anexos = [];
    mencoes = [];
    sugIndex = -1;
    pinned = true;
    try {
      await api.acpPrompt(paneId, text, images, links);
    } finally {
      sending = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    // Com o autocomplete aberto, as setas navegam nele e o Enter escolhe.
    if (sugIndex >= 0 && sugestoes.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        sugIndex = (sugIndex + 1) % sugestoes.length;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        sugIndex = (sugIndex - 1 + sugestoes.length) % sugestoes.length;
        return;
      }
      if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey)) {
        e.preventDefault();
        escolher(sugIndex);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        sugIndex = -1;
        return;
      }
    }
    // Enter envia, Shift+Enter quebra linha — convenção de chat.
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  }

  /** Colar imagem: vira anexo do próximo prompt. */
  async function onPaste(e: ClipboardEvent) {
    const items = Array.from(e.clipboardData?.items ?? []);
    const imagens = items.filter((i) => i.type.startsWith("image/"));
    if (imagens.length === 0) return; // texto normal segue o caminho padrão
    e.preventDefault();
    for (const item of imagens) {
      const blob = item.getAsFile();
      if (!blob) continue;
      const bytes = new Uint8Array(await blob.arrayBuffer());
      anexos = [...anexos, { dataB64: bytesToB64(bytes), mimeType: blob.type }];
    }
  }

  function bytesToB64(bytes: Uint8Array): string {
    let bin = "";
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) {
      bin += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    return btoa(bin);
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

  function trocarModo(id: string) {
    acp.setMode(paneId, id);
    void api.acpSetMode(paneId, id).catch(() => {});
  }

  function trocarModelo(id: string) {
    acp.setModel(paneId, id);
    void api.acpSetModel(paneId, id).catch(() => {});
  }

  /** Contexto consumido, arredondado para o que cabe no rodapé. */
  const usoPct = $derived.by(() => {
    const u = conv.usage;
    if (!u || !u.size) return null;
    return Math.min(100, Math.round((u.used / u.size) * 100));
  });
</script>

<div class="acp" onpointerdown={() => app.setActivePane(paneId)}>
  {#if conv.modes.available.length > 0 || conv.models.available.length > 0}
    <div class="bar">
      {#if conv.modes.available.length > 0}
        <select
          class="sel"
          title={t("acp.mode")}
          value={conv.modes.current}
          onchange={(e) => trocarModo(e.currentTarget.value)}
        >
          {#each conv.modes.available as m (m.id)}
            <option value={m.id}>{m.name}</option>
          {/each}
        </select>
      {/if}
      {#if conv.models.available.length > 0}
        <select
          class="sel"
          title={t("acp.model")}
          value={conv.models.current}
          onchange={(e) => trocarModelo(e.currentTarget.value)}
        >
          {#each conv.models.available as m (m.id)}
            <option value={m.id}>{m.name}</option>
          {/each}
        </select>
      {/if}
      <button
        class="fork"
        title={t("acp.forkHint")}
        disabled={!conv.sessionId || conv.busy}
        onclick={() => app.forkAcpTab(paneId)}
      >
        <GitFork size={11} />
        {t("acp.fork")}
      </button>
      {#if usoPct !== null}
        <span class="uso" class:alto={usoPct >= 80} title={t("acp.contextUsed")}>
          {usoPct}%
        </span>
      {/if}
    </div>
  {/if}

  <div class="log" bind:this={scroller} onscroll={onScroll}>
    {#if !conv.ready && conv.blocks.length === 0}
      <div class="hint">{t("acp.starting")}</div>
    {/if}

    {#each conv.blocks as block (block.id)}
      {#if block.kind === "message"}
        <div class="msg" class:user={block.role === "user"}>
          <span class="who">{block.role === "user" ? t("acp.you") : t("acp.agent")}</span>
          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
          <div class="md">{@html renderMarkdown(block.text)}</div>
        </div>
      {:else if block.kind === "thought"}
        <div class="thought">
          <button
            class="thead"
            onclick={() =>
              (pensamentosAbertos[block.id] = !pensamentosAbertos[block.id])}
          >
            <span class="chev" class:open={pensamentosAbertos[block.id]}>
              <ChevronRight size={11} />
            </span>
            <Brain size={11} />
            <span>{t("acp.thinking")}</span>
          </button>
          {#if pensamentosAbertos[block.id]}
            <div class="tbody">{block.text}</div>
          {/if}
        </div>
      {:else if block.kind === "tool"}
        <AcpToolCard
          {block}
          terminals={conv.terminals}
          onOpenFile={(path, line) => app.openInEditor?.(path, line)}
        />
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

  <div class="composer-wrap">
    {#if sugIndex >= 0 && sugestoes.length > 0}
      <div class="sugs">
        {#each sugestoes as s, i (s.valor)}
          <button
            class="sug"
            class:sel={i === sugIndex}
            onmouseenter={() => (sugIndex = i)}
            onclick={() => escolher(i)}
          >
            <span class="srot">{s.rotulo}</span>
            {#if s.detalhe}<span class="sdet" title={s.detalhe}>{s.detalhe}</span>{/if}
          </button>
        {/each}
      </div>
    {/if}

    {#if anexos.length > 0 || mencoes.length > 0}
      <div class="anexos">
        {#each mencoes as m, i (m.path)}
          <span class="chip" title={m.path}>
            <AtSign size={10} />
            {m.name}
            <button
              class="rmchip"
              title={t("acp.removeMention")}
              onclick={() => (mencoes = mencoes.filter((_, n) => n !== i))}
            >
              <X size={9} />
            </button>
          </span>
        {/each}
        {#each anexos as img, i (i)}
          <div class="anexo">
            <img src={`data:${img.mimeType};base64,${img.dataB64}`} alt="" />
            <button
              class="rm"
              title={t("acp.removeImage")}
              onclick={() => (anexos = anexos.filter((_, n) => n !== i))}
            >
              <X size={10} />
            </button>
          </div>
        {/each}
      </div>
    {/if}

    <div class="composer">
      <textarea
        bind:this={box}
        rows="1"
        bind:value={input}
        onkeydown={onKey}
        onpaste={onPaste}
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
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-bottom: 1px solid #2a2a2a;
    flex: 0 0 auto;
  }
  .sel {
    background: #252526;
    border: 1px solid #3a3d41;
    border-radius: 4px;
    color: #cccccc;
    font: inherit;
    font-size: 10.5px;
    padding: 1px 4px;
    max-width: 150px;
  }
  .sel:focus {
    outline: none;
    border-color: #4a7fb5;
  }
  .fork {
    display: flex;
    align-items: center;
    gap: 4px;
    background: #252526;
    border: 1px solid #3a3d41;
    border-radius: 4px;
    color: #cccccc;
    font: inherit;
    font-size: 10.5px;
    padding: 1px 6px;
    cursor: pointer;
  }
  .fork:hover:not(:disabled) {
    background: #3a3d41;
  }
  .fork:disabled {
    color: #5a5a5a;
    cursor: default;
  }
  .uso {
    margin-left: auto;
    font-size: 10px;
    color: #6a6a6a;
    font-variant-numeric: tabular-nums;
  }
  .uso.alto {
    color: #d9b45f;
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
  .msg.user .md {
    color: #cfd6dd;
    border-left: 2px solid #3a3d41;
    padding-left: 8px;
  }

  /* ── Markdown ─────────────────────────────────────────────────────────── */
  .md :global(p) {
    margin: 0 0 8px;
  }
  .md :global(p:last-child) {
    margin-bottom: 0;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3),
  .md :global(h4) {
    margin: 12px 0 6px;
    font-size: 13px;
    color: #e8e8e8;
  }
  .md :global(ul),
  .md :global(ol) {
    margin: 0 0 8px;
    padding-left: 20px;
  }
  .md :global(li) {
    margin: 2px 0;
  }
  .md :global(code) {
    background: #2a2a2a;
    border-radius: 3px;
    padding: 1px 4px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11.5px;
  }
  .md :global(pre) {
    background: #171717;
    border: 1px solid #2c2c2c;
    border-radius: 5px;
    padding: 8px 10px;
    overflow-x: auto;
    margin: 0 0 8px;
  }
  .md :global(pre code) {
    background: none;
    padding: 0;
    font-size: 11.5px;
    line-height: 1.5;
  }
  .md :global(blockquote) {
    margin: 0 0 8px;
    padding-left: 8px;
    border-left: 2px solid #3a3d41;
    color: #9aa0a6;
  }
  .md :global(a) {
    color: #6ea8fe;
  }
  .md :global(table) {
    border-collapse: collapse;
    margin: 0 0 8px;
    font-size: 11.5px;
  }
  .md :global(th),
  .md :global(td) {
    border: 1px solid #2c2c2c;
    padding: 3px 7px;
    text-align: left;
  }
  .md :global(th) {
    background: #252526;
  }
  .md :global(img) {
    max-width: 100%;
    max-height: 320px;
    border-radius: 5px;
    border: 1px solid #2c2c2c;
    display: block;
    margin: 6px 0;
  }
  .md :global(hr) {
    border: none;
    border-top: 1px solid #2c2c2c;
    margin: 10px 0;
  }

  /* ── Raciocínio ───────────────────────────────────────────────────────── */
  .thead {
    display: flex;
    align-items: center;
    gap: 5px;
    background: none;
    border: none;
    color: #6a6a6a;
    font: inherit;
    font-size: 11px;
    padding: 0;
    cursor: pointer;
  }
  .thead:hover {
    color: #9aa0a6;
  }
  .chev {
    display: flex;
    transition: transform 0.12s ease;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .tbody {
    color: #7b7b7b;
    font-style: italic;
    white-space: pre-wrap;
    border-left: 2px solid #2f2f2f;
    padding-left: 8px;
    margin-top: 4px;
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

  /* ── Composer ─────────────────────────────────────────────────────────── */
  .composer-wrap {
    position: relative;
    flex: 0 0 auto;
    border-top: 1px solid #2a2a2a;
  }
  .sugs {
    position: absolute;
    bottom: 100%;
    left: 12px;
    right: 12px;
    max-height: 240px;
    overflow-y: auto;
    background: #252526;
    border: 1px solid #3a3d41;
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    margin-bottom: 6px;
    z-index: 5;
  }
  /* Duas linhas: nome em cima, descrição embaixo. Descrição de comando é
     prosa longa e não cabe ao lado do nome sem virar reticências inúteis. */
  .sug {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    color: #cccccc;
    padding: 4px 10px;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }
  .sug.sel {
    background: #094771;
  }
  .srot {
    color: #6ea8fe;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    flex: 0 0 auto;
  }
  .sug.sel .srot {
    color: #cfe3ff;
  }
  .sdet {
    color: #8a8a8a;
    font-size: 10.5px;
    line-height: 1.35;
    /* Duas linhas no máximo: a descrição situa, não substitui a documentação. */
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    max-width: 100%;
    word-break: break-word;
  }
  .sug.sel .sdet {
    color: #cfd6dd;
  }
  .anexos {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
    padding: 8px 12px 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    background: #252d38;
    border: 1px solid #35455a;
    border-radius: 3px;
    color: #a9c6ea;
    font-size: 10.5px;
    padding: 1px 4px 1px 5px;
  }
  .rmchip {
    display: flex;
    background: none;
    border: none;
    color: #6a8199;
    cursor: pointer;
    padding: 0 0 0 2px;
  }
  .rmchip:hover {
    color: #e08b8b;
  }
  .anexo {
    position: relative;
  }
  .anexo img {
    height: 48px;
    width: auto;
    max-width: 96px;
    object-fit: cover;
    border-radius: 4px;
    border: 1px solid #3a3d41;
    display: block;
  }
  .rm {
    position: absolute;
    top: -5px;
    right: -5px;
    width: 15px;
    height: 15px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #3a3d41;
    border: 1px solid #1e1e1e;
    border-radius: 50%;
    color: #ddd;
    cursor: pointer;
    padding: 0;
  }
  .rm:hover {
    background: #6b4a4a;
  }
  .composer {
    display: flex;
    align-items: flex-end;
    gap: 6px;
    padding: 8px 12px;
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
