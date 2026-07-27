<script lang="ts">
  import { onMount } from "svelte";
  import { X } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { profile } from "../lib/profiles";
  import ToolIcon from "./ToolIcon.svelte";
  import type { UsageStats } from "../lib/types";

  let stats = $state<UsageStats[]>([]);
  let loading = $state(true);

  const total = $derived(
    stats.reduce(
      (a, s) => ({
        sessions: a.sessions + s.sessions,
        input: a.input + s.input,
        output: a.output + s.output,
        cost: a.cost + s.cost,
      }),
      { sessions: 0, input: 0, output: 0, cost: 0 },
    ),
  );

  onMount(async () => {
    try {
      stats = await api.usageLoad();
    } finally {
      loading = false;
    }
  });

  function fmt(n: number): string {
    if (n >= 1e9) return (n / 1e9).toFixed(2) + "B";
    if (n >= 1e6) return (n / 1e6).toFixed(1) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "K";
    return String(n);
  }
</script>

<div class="backdrop" onclick={() => (app.usageOpen = false)} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
    <div class="head">
      <h2>Uso de tokens</h2>
      <button class="close" onclick={() => (app.usageOpen = false)}><X size={16} /></button>
    </div>

    {#if loading}
      <div class="muted">Calculando… (a primeira vez varre os arquivos das CLIs)</div>
    {:else}
      <div class="cards">
        {#each stats as s (s.harness)}
          {@const P = profile(s.harness)}
          <div class="card" style="--c:{P.color}">
            <div class="ct">
              <span class="ic"><ToolIcon id={s.harness} size={14} /></span>{P.label}
            </div>
            <div class="big">{fmt(s.input + s.output)}</div>
            <div class="sub">tokens · {s.sessions} sessões</div>
            <div class="io">
              <span>↑ {fmt(s.input)}</span><span>↓ {fmt(s.output)}</span>
              {#if s.cost > 0}<span>${s.cost.toFixed(2)}</span>{/if}
            </div>
          </div>
        {/each}
      </div>
      <div class="total">
        Total: <b>{fmt(total.input + total.output)}</b> tokens · {total.sessions} sessões
        {#if total.cost > 0}· ${total.cost.toFixed(2)}{/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 560px;
    max-width: 92vw;
    background: #252526;
    color: #d4d4d4;
    border: 1px solid #3a3a3a;
    border-radius: 8px;
    padding: 16px 20px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }
  h2 {
    font-size: 16px;
    margin: 0;
  }
  .close {
    background: none;
    border: none;
    color: #888;
    font-size: 14px;
    cursor: pointer;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }
  .card {
    background: #1e1e1e;
    border: 1px solid #333;
    border-top: 2px solid var(--c);
    border-radius: 8px;
    padding: 12px;
  }
  .ct {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #b8b8b8;
  }
  .ic {
    color: var(--c);
  }
  .big {
    font-size: 22px;
    font-weight: 700;
    margin-top: 6px;
  }
  .sub {
    font-size: 11px;
    color: #8a8a8a;
  }
  .io {
    display: flex;
    gap: 10px;
    margin-top: 8px;
    font-size: 11px;
    color: #9aa0a6;
  }
  .total {
    margin-top: 14px;
    padding-top: 10px;
    border-top: 1px solid #333;
    font-size: 13px;
    color: #b8b8b8;
  }
  .muted {
    color: #7a7a7a;
    padding: 20px 0;
    text-align: center;
  }
</style>
