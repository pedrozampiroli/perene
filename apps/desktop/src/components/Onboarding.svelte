<script lang="ts">
  // Tour de boas-vindas com SPOTLIGHT: recorta a luz em cima do elemento real da
  // UI (via `data-tour="…"`), aponta uma setinha e anima a transição entre os
  // passos. Passos sem alvo (ou cujo alvo não existe na tela) caem num card
  // centralizado — assim o tour nunca quebra.
  import {
    Sparkles,
    Infinity as InfinityIcon,
    LayoutGrid,
    FolderTree,
    GitBranch,
    Code2,
    Keyboard,
    ChevronLeft,
    ChevronRight,
    Lightbulb,
    X,
  } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { PROFILES } from "../lib/profiles";
  import ToolIcon from "./ToolIcon.svelte";

  interface Step {
    key: string;
    icon: typeof Sparkles;
    color: string;
    /** Elemento real destacado (data-tour). Sem alvo → card centralizado. */
    target?: string;
  }

  const STEPS: Step[] = [
    { key: "welcome", icon: Sparkles, color: "#d97557" },
    { key: "sessions", icon: InfinityIcon, color: "#4ec9b0" },
    { key: "profiles", icon: LayoutGrid, color: "#0fa37f", target: "profiles" },
    { key: "organize", icon: FolderTree, color: "#5c8cfa", target: "tabs" },
    { key: "splits", icon: LayoutGrid, color: "#c586c0", target: "splits" },
    { key: "editor", icon: Code2, color: "#e2c08d", target: "editor" },
    { key: "git", icon: GitBranch, color: "#d4a72c", target: "git" },
    { key: "shortcuts", icon: Keyboard, color: "#bc8cff", target: "settings" },
  ];

  const SHORTCUTS: [string, string][] = [
    ["⌘T", "shortcuts.newTerminal"],
    ["⌘D", "shortcuts.splitRight"],
    ["⌘P", "search.quickOpen"],
    ["⌘⇧F", "search.globalSearch"],
    ["⌘Y", "shortcuts.history"],
    ["⌘U", "shortcuts.usage"],
  ];

  const PAD = 8; // respiro do recorte em volta do elemento
  const GAP = 14; // distância do balão até o alvo

  let step = $state(0);
  let rect = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let place = $state<"top" | "bottom" | "left" | "right" | "center">("center");
  let callout = $state<{ x: number; y: number } | null>(null);
  let calloutEl = $state<HTMLElement>();

  const cur = $derived(STEPS[step]);
  const isLast = $derived(step === STEPS.length - 1);

  /** Mede o alvo do passo e decide onde encaixar o balão. */
  function measure() {
    const s = STEPS[step];
    const el = s.target ? document.querySelector<HTMLElement>(`[data-tour="${s.target}"]`) : null;
    if (!el) {
      rect = null;
      place = "center";
      callout = null;
      return;
    }
    const r = el.getBoundingClientRect();
    if (r.width === 0 || r.height === 0) {
      rect = null;
      place = "center";
      callout = null;
      return;
    }
    rect = { x: r.left - PAD, y: r.top - PAD, w: r.width + PAD * 2, h: r.height + PAD * 2 };

    // Escolhe o lado com mais espaço livre.
    const cw = calloutEl?.offsetWidth ?? 380;
    const ch = calloutEl?.offsetHeight ?? 260;
    const space = {
      top: r.top,
      bottom: window.innerHeight - r.bottom,
      left: r.left,
      right: window.innerWidth - r.right,
    };
    if (space.top >= ch + GAP) place = "top";
    else if (space.bottom >= ch + GAP) place = "bottom";
    else if (space.right >= cw + GAP) place = "right";
    else if (space.left >= cw + GAP) place = "left";
    else place = "center";

    const cx = r.left + r.width / 2;
    const cy = r.top + r.height / 2;
    let x = cx - cw / 2;
    let y = cy - ch / 2;
    if (place === "top") y = r.top - ch - GAP;
    if (place === "bottom") y = r.bottom + GAP;
    if (place === "left") x = r.left - cw - GAP;
    if (place === "right") x = r.right + GAP;
    // Mantém o balão dentro da janela.
    x = Math.max(12, Math.min(x, window.innerWidth - cw - 12));
    y = Math.max(12, Math.min(y, window.innerHeight - ch - 12));
    callout = place === "center" ? null : { x, y };
  }

  // Remede ao trocar de passo, redimensionar a janela e após o layout assentar.
  $effect(() => {
    void step;
    void app.onboardingOpen;
    if (!app.onboardingOpen) return;
    measure();
    const id = setTimeout(measure, 60); // 2ª medida com o balão já renderizado
    return () => clearTimeout(id);
  });

  /** Posição da setinha, relativa ao balão, apontando para o centro do alvo. */
  const arrow = $derived.by(() => {
    if (!rect || !callout || place === "center") return null;
    const cx = rect.x + rect.w / 2;
    const cy = rect.y + rect.h / 2;
    const cw = calloutEl?.offsetWidth ?? 380;
    const ch = calloutEl?.offsetHeight ?? 260;
    if (place === "top") return { left: Math.max(16, Math.min(cx - callout.x, cw - 16)), top: ch };
    if (place === "bottom") return { left: Math.max(16, Math.min(cx - callout.x, cw - 16)), top: 0 };
    if (place === "left") return { left: cw, top: Math.max(16, Math.min(cy - callout.y, ch - 16)) };
    return { left: 0, top: Math.max(16, Math.min(cy - callout.y, ch - 16)) };
  });

  function next() {
    if (isLast) app.finishOnboarding();
    else step++;
  }
  function back() {
    if (step > 0) step--;
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") app.finishOnboarding();
    else if (e.key === "ArrowRight" || e.key === "Enter") next();
    else if (e.key === "ArrowLeft") back();
  }
</script>

<svelte:window onkeydown={app.onboardingOpen ? onKey : undefined} onresize={measure} />

{#if app.onboardingOpen}
  {@const Icon = cur.icon}

  <!-- Escurece a tela. Com alvo, o "buraco" é o próprio spotlight (box-shadow). -->
  {#if rect}
    <div class="spot" style="left:{rect.x}px; top:{rect.y}px; width:{rect.w}px; height:{rect.h}px"></div>
  {:else}
    <div class="dim"></div>
  {/if}

  <div
    class="callout"
    class:centered={!callout}
    class:anchored={!!callout}
    style={callout ? `left:${callout.x}px; top:${callout.y}px` : ""}
    bind:this={calloutEl}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    {#if arrow}
      <span class="arrow {place}" style="left:{arrow.left}px; top:{arrow.top}px"></span>
    {/if}

    <button class="close" title={t("onb.skip")} onclick={() => app.finishOnboarding()}>
      <X size={15} />
    </button>

    <div class="head">
      <span class="hero" style="--c:{cur.color}"><Icon size={20} /></span>
      <h2>{t(`onb.${cur.key}.title`)}</h2>
    </div>

    <p class="body">{t(`onb.${cur.key}.body`)}</p>

    {#if cur.key === "profiles"}
      <div class="showcase">
        {#each PROFILES as p (p.id)}
          <div class="chip" style="--c:{p.color}">
            <ToolIcon id={p.id} size={16} />
            <span>{p.label}</span>
          </div>
        {/each}
      </div>
    {:else if cur.key === "shortcuts"}
      <div class="keys">
        {#each SHORTCUTS as [k, label] (k)}
          <div class="krow"><kbd>{k}</kbd><span>{t(label)}</span></div>
        {/each}
      </div>
    {/if}

    <div class="tip">
      <Lightbulb size={13} />
      <span>{t(`onb.${cur.key}.tip`)}</span>
    </div>

    <div class="foot">
      <div class="dots">
        {#each STEPS as s, i (s.key)}
          <button class="dot" class:on={i === step} aria-label={t("onb.step", { current: i + 1, total: STEPS.length })} onclick={() => (step = i)}></button>
        {/each}
      </div>
      <div class="actions">
        {#if step > 0}
          <button class="ghost" onclick={back}><ChevronLeft size={14} /> {t("onb.back")}</button>
        {:else}
          <button class="ghost" onclick={() => app.finishOnboarding()}>{t("onb.skip")}</button>
        {/if}
        <button class="primary" onclick={next}>
          {isLast ? t("onb.start") : t("onb.next")}
          {#if !isLast}<ChevronRight size={14} />{/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dim,
  .spot {
    position: fixed;
    z-index: 500;
    pointer-events: none;
  }
  .dim {
    inset: 0;
    background: rgba(0, 0, 0, 0.62);
    animation: fade 0.25s ease;
  }
  /* O recorte de luz: a sombra gigante escurece TUDO menos este retângulo. */
  .spot {
    border-radius: 10px;
    box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.62);
    transition:
      left 0.4s cubic-bezier(0.4, 0, 0.2, 1),
      top 0.4s cubic-bezier(0.4, 0, 0.2, 1),
      width 0.4s cubic-bezier(0.4, 0, 0.2, 1),
      height 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }
  /* Anel pulsante em volta do alvo. */
  .spot::after {
    content: "";
    position: absolute;
    inset: -3px;
    border: 2px solid #4ea1ff;
    border-radius: 12px;
    animation: pulse 1.9s ease-out infinite;
  }
  @keyframes pulse {
    0% {
      box-shadow: 0 0 0 0 rgba(78, 161, 255, 0.55);
      opacity: 1;
    }
    70% {
      box-shadow: 0 0 0 14px rgba(78, 161, 255, 0);
      opacity: 0.85;
    }
    100% {
      box-shadow: 0 0 0 0 rgba(78, 161, 255, 0);
      opacity: 1;
    }
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(6px);
    }
  }

  .callout {
    position: fixed;
    z-index: 501;
    width: 380px;
    max-width: 92vw;
    background: #252526;
    color: #d4d4d4;
    border: 1px solid #3f3f46;
    border-radius: 12px;
    padding: 18px 20px 14px;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.65);
    animation: pop 0.28s cubic-bezier(0.34, 1.4, 0.64, 1);
  }
  .callout.anchored {
    transition:
      left 0.4s cubic-bezier(0.4, 0, 0.2, 1),
      top 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  }
  .callout.centered {
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 460px;
    text-align: center;
  }
  .callout.centered .head {
    flex-direction: column;
    gap: 10px;
  }
  .callout.centered .tip {
    text-align: left;
  }

  /* Setinha (losango girado) grudada na borda que aponta pro alvo. */
  .arrow {
    position: absolute;
    width: 12px;
    height: 12px;
    background: #252526;
    border: 1px solid #3f3f46;
    transform: rotate(45deg);
    margin: -6px 0 0 -6px;
  }
  .arrow.top {
    border-left: none;
    border-top: none;
  }
  .arrow.bottom {
    border-right: none;
    border-bottom: none;
  }
  .arrow.left {
    border-left: none;
    border-bottom: none;
  }
  .arrow.right {
    border-right: none;
    border-top: none;
  }

  .close {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    background: none;
    border: none;
    color: #6a6a6a;
    cursor: pointer;
    padding: 3px;
    border-radius: 5px;
  }
  .close:hover {
    color: #ddd;
    background: #3a3d41;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 11px;
    margin-bottom: 10px;
  }
  .hero {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    flex: 0 0 auto;
    border-radius: 11px;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--c) 38%, transparent);
  }
  h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: #fff;
  }
  .body {
    margin: 0;
    font-size: 13px;
    line-height: 1.6;
    color: #c2c2c2;
  }
  .showcase {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: 14px;
    justify-content: center;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 8px;
    background: #1e1e1e;
    border: 1px solid #333;
    border-top: 2px solid var(--c);
    color: var(--c);
    font-size: 11.5px;
  }
  .chip span {
    color: #cfcfcf;
  }
  .keys {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 5px 12px;
    margin-top: 14px;
    text-align: left;
  }
  .krow {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 11.5px;
    color: #b8b8b8;
    min-width: 0;
  }
  .krow span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  kbd {
    flex: 0 0 auto;
    min-width: 38px;
    text-align: center;
    background: #1e1e1e;
    border: 1px solid #444;
    border-radius: 5px;
    padding: 2px 5px;
    font-family: Menlo, monospace;
    font-size: 10.5px;
    color: #ddd;
  }
  .tip {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    margin: 14px 0 2px;
    padding: 9px 11px;
    background: #1e1e1e;
    border-left: 2px solid #d4a72c;
    border-radius: 6px;
    font-size: 11.5px;
    line-height: 1.5;
    color: #a9a9a9;
    text-align: left;
  }
  .tip :global(svg) {
    color: #d4a72c;
    flex: 0 0 auto;
    margin-top: 1px;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid #333;
  }
  .dots {
    display: flex;
    gap: 5px;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    border: none;
    background: #4a4a4a;
    cursor: pointer;
    padding: 0;
    transition: all 0.25s ease;
  }
  .dot.on {
    background: #4ea1ff;
    width: 16px;
    border-radius: 3px;
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .actions button {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    border: none;
    padding: 7px 14px;
    border-radius: 7px;
    cursor: pointer;
    font-size: 12.5px;
  }
  .ghost {
    background: none;
    color: #9aa0a6;
  }
  .ghost:hover {
    background: #3a3d41;
    color: #ddd;
  }
  .primary {
    background: #0e639c;
    color: #fff;
  }
  .primary:hover {
    background: #1177bb;
  }
</style>
