<script lang="ts">
  // Indicador do que a sessão está fazendo. Discreto por princípio: ocupa 8px,
  // não desloca nada e some sozinho quando não há o que dizer.
  //
  //   running → arco girando (herda a cor da marca, translúcido)
  //   waiting → âmbar respirando (pede atenção sem gritar)
  //   done    → verde aceso, some sozinho depois de um tempo
  //   error   → vermelho aceso
  import type { PaneState } from "../lib/types";
  import { t } from "../lib/i18n.svelte";

  let { state, size = 8 }: { state: PaneState | null | undefined; size?: number } = $props();

  const label = $derived(state ? t(`status.${state}`) : "");
</script>

{#if state && state !== "idle"}
  <span class="dot {state}" style="--s:{size}px" title={label} aria-label={label}></span>
{/if}

<style>
  .dot {
    display: inline-block;
    width: var(--s);
    height: var(--s);
    border-radius: 50%;
    flex: 0 0 auto;
  }

  /* Rodando: anel fino girando, na cor herdada do contexto (a da marca). */
  .running {
    border-radius: 50%;
    border: 1.5px solid currentColor;
    border-top-color: transparent;
    border-right-color: transparent;
    opacity: 0.85;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Esperando aprovação: respira (mais suave que piscar). */
  .waiting {
    background: #e2b341;
    box-shadow: 0 0 0 0 rgba(226, 179, 65, 0.5);
    animation: breathe 1.6s ease-in-out infinite;
  }
  @keyframes breathe {
    0%,
    100% {
      opacity: 1;
      box-shadow: 0 0 0 0 rgba(226, 179, 65, 0.45);
    }
    50% {
      opacity: 0.55;
      box-shadow: 0 0 0 3px rgba(226, 179, 65, 0);
    }
  }

  /* Terminou: entra suave e fica aceso (o daemon apaga sozinho depois). */
  .done {
    background: #3fb950;
    animation: appear 0.35s ease;
  }
  .error {
    background: #f14c4c;
    animation: appear 0.35s ease;
  }
  @keyframes appear {
    from {
      opacity: 0;
      transform: scale(0.4);
    }
  }

  /* Respeita quem prefere menos movimento. */
  @media (prefers-reduced-motion: reduce) {
    .running,
    .waiting {
      animation: none;
    }
    .running {
      border-color: currentColor;
    }
  }
</style>
