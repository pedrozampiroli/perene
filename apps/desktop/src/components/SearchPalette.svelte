<script lang="ts">
  // Palette estilo VSCodium/Zed: ⌘P (ir para arquivo), ⌘⇧F (buscar nos arquivos),
  // ⌘⇧H (substituir nos arquivos).
  import { Search, FileText, Replace } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import { baseName } from "../lib/paths";

  let sel = $state(0);
  let debounce: ReturnType<typeof setTimeout>;

  /** Fuzzy simples: todas as letras da query aparecem em ordem no caminho. */
  function fuzzy(files: string[], q: string): string[] {
    const query = q.trim().toLowerCase();
    if (!query) return files.slice(0, 200);
    const scored: { f: string; score: number }[] = [];
    for (const f of files) {
      const s = f.toLowerCase();
      let i = 0;
      let score = 0;
      let lastHit = -1;
      for (const ch of query) {
        const idx = s.indexOf(ch, i);
        if (idx < 0) {
          score = -1;
          break;
        }
        score += idx === lastHit + 1 ? 3 : 1; // premia letras seguidas
        lastHit = idx;
        i = idx + 1;
      }
      if (score < 0) continue;
      if (baseName(s).includes(query)) score += 40; // nome do arquivo bate
      scored.push({ f, score });
    }
    scored.sort((a, b) => b.score - a.score || a.f.length - b.f.length);
    return scored.slice(0, 200).map((x) => x.f);
  }

  const p = $derived(app.palette);
  const matches = $derived(p?.mode === "quickOpen" ? fuzzy(p.files, p.query) : []);
  const rows = $derived(p?.mode === "quickOpen" ? matches.length : (p?.hits.length ?? 0));

  function onInput() {
    sel = 0;
    if (!p) return;
    if (p.mode !== "quickOpen") {
      clearTimeout(debounce);
      debounce = setTimeout(() => app.runSearch(), 280);
    }
  }

  function choose(i: number) {
    if (!p) return;
    if (p.mode === "quickOpen") {
      const f = matches[i];
      if (f) app.openHit(f);
    } else {
      const h = p.hits[i];
      if (h) app.openHit(h.path, h.line);
    }
  }

  function onKey(e: KeyboardEvent) {
    if (!p) return;
    if (e.key === "Escape") {
      app.palette = null;
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      sel = Math.min(sel + 1, Math.max(0, rows - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      sel = Math.max(0, sel - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (p.mode === "search" && !p.hits.length) app.runSearch();
      else choose(sel);
    }
  }
</script>

{#if p}
  <div class="backdrop" onclick={() => (app.palette = null)} role="presentation">
    <div class="palette" onclick={(e) => e.stopPropagation()} onkeydown={onKey} role="dialog" aria-modal="true" tabindex="-1">
      <div class="field">
        <span class="ic">
          {#if p.mode === "quickOpen"}<FileText size={15} />{:else if p.mode === "replace"}<Replace size={15} />{:else}<Search size={15} />{/if}
        </span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          bind:value={p.query}
          oninput={onInput}
          autofocus
          placeholder={p.mode === "quickOpen" ? t("search.filePlaceholder") : t("search.placeholder")}
        />
      </div>

      {#if p.mode === "replace"}
        <div class="field">
          <span class="ic"><Replace size={15} /></span>
          <input bind:value={p.replacement} placeholder={t("search.replacePlaceholder")} />
          <button class="apply" disabled={!p.hits.length || p.loading} onclick={() => app.runReplace()}>
            {t("search.replaceAll")}
          </button>
        </div>
      {/if}

      <div class="results">
        {#if p.loading}
          <div class="muted">{t("search.searching")}</div>
        {:else if p.mode === "quickOpen"}
          {#each matches as f, i (f)}
            <button class="row" class:sel={i === sel} onclick={() => choose(i)} onmouseenter={() => (sel = i)}>
              <span class="name">{baseName(f)}</span>
              <span class="path">{f}</span>
            </button>
          {:else}
            <div class="muted">{t("search.noResults")}</div>
          {/each}
        {:else}
          {#each p.hits as h, i (h.path + ":" + h.line + ":" + i)}
            <button class="row" class:sel={i === sel} onclick={() => choose(i)} onmouseenter={() => (sel = i)}>
              <span class="loc">{h.path}:{h.line}</span>
              <span class="text">{h.text}</span>
            </button>
          {:else}
            {#if p.message}<div class="muted">{p.message}</div>{/if}
          {/each}
        {/if}
      </div>

      <div class="foot">
        {#if p.mode !== "quickOpen" && p.hits.length}
          {t("search.results", { count: p.hits.length })}
        {:else if p.mode === "quickOpen" && matches.length}
          {t("search.results", { count: matches.length })}
        {/if}
        <span class="hint">↑↓ · Enter · Esc</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    z-index: 250;
  }
  .palette {
    width: 620px;
    max-width: 92vw;
    background: #252526;
    border: 1px solid #3a3a3a;
    border-radius: 10px;
    box-shadow: 0 16px 50px rgba(0, 0, 0, 0.6);
    overflow: hidden;
    color: #d4d4d4;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-bottom: 1px solid #333;
  }
  .ic {
    display: flex;
    color: #8a8a8a;
    flex: 0 0 auto;
  }
  input {
    flex: 1 1 auto;
    min-width: 0;
    background: none;
    border: none;
    color: #fff;
    font-size: 14px;
    outline: none;
  }
  .apply {
    flex: 0 0 auto;
    background: #0e639c;
    border: none;
    color: #fff;
    padding: 5px 10px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 12px;
  }
  .apply:disabled {
    background: #333;
    color: #777;
    cursor: default;
  }
  .results {
    max-height: 46vh;
    overflow-y: auto;
    padding: 4px;
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    background: none;
    border: none;
    color: #cccccc;
    text-align: left;
    padding: 5px 9px;
    border-radius: 5px;
    cursor: pointer;
    font-size: 12.5px;
    overflow: hidden;
  }
  .row.sel {
    background: #04395e;
    color: #fff;
  }
  .name {
    flex: 0 0 auto;
    font-weight: 500;
  }
  .path,
  .text {
    color: #8a8a8a;
    font-size: 11.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .loc {
    flex: 0 0 auto;
    color: #6ea8fe;
    font-size: 11.5px;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text {
    font-family: Menlo, monospace;
  }
  .muted {
    color: #7a7a7a;
    padding: 10px;
    font-size: 12.5px;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    padding: 6px 12px;
    border-top: 1px solid #333;
    font-size: 11px;
    color: #7a7a7a;
  }
</style>
