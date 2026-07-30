<script lang="ts">
  // Tour de boas-vindas: abre na primeira execução (settings.onboardingDone) e
  // pode ser revisto pelas configurações.
  import {
    Sparkles,
    Infinity as InfinityIcon,
    LayoutGrid,
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

  let step = $state(0);

  const STEPS = [
    { key: "welcome", icon: Sparkles, color: "#d97557" },
    { key: "sessions", icon: InfinityIcon, color: "#4ec9b0" },
    { key: "profiles", icon: LayoutGrid, color: "#0fa37f" },
    { key: "organize", icon: GitBranch, color: "#5c8cfa" },
    { key: "editor", icon: Code2, color: "#e2c08d" },
    { key: "shortcuts", icon: Keyboard, color: "#bc8cff" },
  ];

  const cur = $derived(STEPS[step]);
  const isLast = $derived(step === STEPS.length - 1);

  // Atalhos exibidos no último passo.
  const SHORTCUTS: [string, string][] = [
    ["⌘T", "shortcuts.newTerminal"],
    ["⌘D", "shortcuts.splitRight"],
    ["⌘P", "search.quickOpen"],
    ["⌘⇧F", "search.globalSearch"],
    ["⌘Y", "shortcuts.history"],
    ["⌘U", "shortcuts.usage"],
  ];

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

<svelte:window onkeydown={app.onboardingOpen ? onKey : undefined} />

{#if app.onboardingOpen}
  {@const Icon = cur.icon}
  <div class="backdrop" role="presentation">
    <div class="modal" role="dialog" aria-modal="true" tabindex="-1">
      <button class="close" title={t("onb.skip")} onclick={() => app.finishOnboarding()}>
        <X size={16} />
      </button>

      <div class="hero" style="--c:{cur.color}">
        <Icon size={30} />
      </div>

      <h2>{t(`onb.${cur.key}.title`)}</h2>
      <p class="body">{t(`onb.${cur.key}.body`)}</p>

      <!-- Ilustração por passo -->
      {#if cur.key === "profiles"}
        <div class="showcase">
          {#each PROFILES as p (p.id)}
            <div class="chip" style="--c:{p.color}">
              <ToolIcon id={p.id} size={18} />
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
        <Lightbulb size={14} />
        <span>{t(`onb.${cur.key}.tip`)}</span>
      </div>

      <div class="foot">
        <div class="dots">
          {#each STEPS as s, i (s.key)}
            <button
              class="dot"
              class:on={i === step}
              aria-label={t("onb.step", { current: i + 1, total: STEPS.length })}
              onclick={() => (step = i)}
            ></button>
          {/each}
        </div>
        <div class="actions">
          {#if step > 0}
            <button class="ghost" onclick={back}><ChevronLeft size={15} /> {t("onb.back")}</button>
          {:else}
            <button class="ghost" onclick={() => app.finishOnboarding()}>{t("onb.skip")}</button>
          {/if}
          <button class="primary" onclick={next}>
            {isLast ? t("onb.start") : t("onb.next")}
            {#if !isLast}<ChevronRight size={15} />{/if}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(3px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 500;
  }
  .modal {
    position: relative;
    width: 520px;
    max-width: 92vw;
    background: #252526;
    color: #d4d4d4;
    border: 1px solid #3a3a3a;
    border-radius: 12px;
    padding: 26px 28px 20px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
    text-align: center;
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    display: flex;
    background: none;
    border: none;
    color: #6a6a6a;
    cursor: pointer;
    padding: 4px;
    border-radius: 5px;
  }
  .close:hover {
    color: #ddd;
    background: #3a3d41;
  }
  .hero {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 62px;
    height: 62px;
    margin: 4px auto 16px;
    border-radius: 16px;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--c) 35%, transparent);
  }
  h2 {
    margin: 0 0 10px;
    font-size: 19px;
    font-weight: 600;
    color: #fff;
  }
  .body {
    margin: 0 auto;
    max-width: 420px;
    font-size: 13.5px;
    line-height: 1.6;
    color: #c2c2c2;
  }
  .showcase {
    display: flex;
    justify-content: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 18px;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 12px;
    border-radius: 8px;
    background: #1e1e1e;
    border: 1px solid #333;
    border-top: 2px solid var(--c);
    color: var(--c);
    font-size: 12px;
  }
  .chip span {
    color: #cfcfcf;
  }
  .keys {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px 14px;
    margin-top: 18px;
    text-align: left;
  }
  .krow {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
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
    min-width: 42px;
    text-align: center;
    background: #1e1e1e;
    border: 1px solid #444;
    border-radius: 5px;
    padding: 2px 6px;
    font-family: Menlo, monospace;
    font-size: 11px;
    color: #ddd;
  }
  .tip {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 20px 0 4px;
    padding: 10px 12px;
    background: #1e1e1e;
    border-left: 2px solid #d4a72c;
    border-radius: 6px;
    font-size: 12px;
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
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid #333;
  }
  .dots {
    display: flex;
    gap: 6px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    border: none;
    background: #4a4a4a;
    cursor: pointer;
    padding: 0;
  }
  .dot.on {
    background: #0e639c;
    width: 18px;
    border-radius: 4px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .actions button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: none;
    padding: 8px 16px;
    border-radius: 7px;
    cursor: pointer;
    font-size: 13px;
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
