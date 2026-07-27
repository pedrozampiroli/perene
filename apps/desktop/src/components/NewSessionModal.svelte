<script lang="ts">
  import { GitBranch, FolderOpen, FolderGit2 } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { profile } from "../lib/profiles";

  const prof = $derived(app.newSession ? profile(app.newSession.profileId) : profile("shell"));

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") app.newSession = null;
    else if (e.key === "Enter" && app.newSession?.mode !== "new") app.confirmNewSession();
  }
</script>

{#if app.newSession}
  {@const s = app.newSession}
  <div class="backdrop" onclick={() => (app.newSession = null)} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={onKey} role="dialog" aria-modal="true" tabindex="-1">
      <h3>Nova sessão — {prof.label}</h3>

      <label class="opt" class:sel={s.mode === "project"}>
        <input type="radio" checked={s.mode === "project"} onchange={() => (s.mode = "project")} />
        <div>
          <div class="t"><FolderOpen size={14} /> No diretório do projeto</div>
          <div class="sub">Abre a sessão direto no diretório do workspace.</div>
        </div>
      </label>

      {#if s.worktrees.length}
        <label class="opt" class:sel={s.mode === "existing"}>
          <input type="radio" checked={s.mode === "existing"} onchange={() => (s.mode = "existing")} />
          <div>
            <div class="t"><FolderGit2 size={14} /> Em uma worktree existente</div>
            <div class="sub">Entra numa worktree que já existe — útil pra acompanhar o que a IA está fazendo.</div>
          </div>
        </label>

        {#if s.mode === "existing"}
          <div class="wt">
            <label class="field">
              <span>Worktree</span>
              <select bind:value={s.existingPath}>
                {#each s.worktrees as w (w.path)}
                  <option value={w.path}>{w.branch || "(detached)"} — {w.path.split("/").slice(-1)[0]}</option>
                {/each}
              </select>
            </label>
          </div>
        {/if}
      {/if}

      <label class="opt" class:sel={s.mode === "new"}>
        <input type="radio" checked={s.mode === "new"} onchange={() => (s.mode = "new")} />
        <div>
          <div class="t"><GitBranch size={14} /> Em uma worktree nova</div>
          <div class="sub">Cria um branch/worktree novo em <code>.perene/worktrees/</code> (ignorado no git).</div>
        </div>
      </label>

      {#if s.mode === "new"}
        <div class="wt">
          <label class="field">
            <span>Baseada no branch</span>
            <select bind:value={s.base}>
              {#each s.branches as b (b)}<option value={b}>{b}</option>{/each}
            </select>
          </label>
          <label class="field">
            <span>Nome (branch + pasta)</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input bind:value={s.name} placeholder="ex.: fix-login" autofocus />
          </label>
        </div>
      {/if}

      {#if s.error}<div class="err">{s.error}</div>{/if}

      <label class="always">
        <input type="checkbox" checked={!app.settings.askWorktree} onchange={(e) => app.setAskWorktree(!e.currentTarget.checked)} />
        Não perguntar de novo (usar sempre o diretório do projeto)
      </label>

      <div class="actions">
        <button class="cancel" onclick={() => (app.newSession = null)}>Cancelar</button>
        <button
          class="ok"
          disabled={s.creating ||
            (s.mode === "new" && !s.name.trim()) ||
            (s.mode === "existing" && !s.existingPath)}
          onclick={() => app.confirmNewSession()}
        >
          {s.creating ? "Criando…" : "Criar sessão"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .modal {
    width: 440px;
    max-width: 92vw;
    background: #252526;
    color: #d4d4d4;
    border: 1px solid #3a3a3a;
    border-radius: 8px;
    padding: 18px 20px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  }
  h3 {
    margin: 0 0 14px;
    font-size: 15px;
  }
  .opt {
    display: flex;
    gap: 10px;
    padding: 10px;
    border: 1px solid #333;
    border-radius: 8px;
    margin-bottom: 8px;
    cursor: pointer;
  }
  .opt.sel {
    border-color: #0e639c;
    background: #0e639c1a;
  }
  .opt input {
    margin-top: 3px;
  }
  .t {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13.5px;
  }
  .sub {
    font-size: 11.5px;
    color: #8a8a8a;
    margin-top: 3px;
  }
  code {
    background: #1e1e1e;
    padding: 0 4px;
    border-radius: 3px;
    font-size: 11px;
  }
  .wt {
    padding: 4px 4px 4px 34px;
  }
  .field {
    display: block;
    margin-bottom: 8px;
  }
  .field span {
    display: block;
    font-size: 11px;
    color: #8a8a8a;
    margin-bottom: 4px;
  }
  select,
  .field input {
    width: 100%;
    box-sizing: border-box;
    background: #1e1e1e;
    border: 1px solid #3a3a3a;
    color: #fff;
    padding: 6px 8px;
    border-radius: 6px;
    outline: none;
    font-size: 13px;
  }
  .err {
    color: #f14c4c;
    font-size: 12px;
    margin: 4px 0;
    word-break: break-word;
  }
  .always {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11.5px;
    color: #9aa0a6;
    margin-top: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .actions button {
    border: none;
    padding: 7px 16px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
  }
  .cancel {
    background: #3a3d41;
    color: #ddd;
  }
  .ok {
    background: #0e639c;
    color: #fff;
  }
  .ok:disabled {
    background: #333;
    color: #777;
    cursor: default;
  }
</style>
