<script lang="ts">
  // MCP e skills das ferramentas de IA.
  //
  // Uma aba por harness. Os arquivos são do usuário (~/.claude.json,
  // ~/.codex/config.toml, opencode.json) — o backend preserva tudo que não é
  // MCP, e ligar/desligar guarda a config inteira em vez de apagá-la.

  import { onMount } from "svelte";
  import { X, Plus, Trash2, Copy, FolderOpen } from "@lucide/svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { profile } from "../lib/profiles";
  import ToolIcon from "./ToolIcon.svelte";
  import type { HarnessId, HarnessInfo, McpServer, Skill } from "../lib/types";

  const TABS: HarnessId[] = ["claude", "codex", "opencode"];

  let tab = $state<HarnessId>("claude");
  let harnesses = $state<HarnessInfo[]>([]);
  let skills = $state<Skill[]>([]);
  let error = $state("");
  let busy = $state(false);

  // Formulário de novo servidor.
  let adding = $state(false);
  let form = $state({ name: "", command: "", args: "", url: "", env: "" });

  const current = $derived(harnesses.find((h) => h.id === tab));
  // Skills de projeto entram no cwd do pane ativo (que pode ser uma worktree).
  const projectRoot = $derived(app.currentDir);

  onMount(refresh);

  // Cada ferramenta lê diretórios diferentes, então trocar de aba precisa
  // recarregar — senão a lista fica congelada na primeira.
  $effect(() => {
    void reloadSkills(tab);
  });

  async function reloadSkills(h: HarnessId): Promise<void> {
    try {
      skills = await api.skillsList(h, projectRoot || undefined);
    } catch (e) {
      error = String(e);
    }
  }

  async function refresh(): Promise<void> {
    error = "";
    try {
      harnesses = await api.harnessList();
      await reloadSkills(tab);
    } catch (e) {
      error = String(e);
    }
  }

  function resetForm(): void {
    form = { name: "", command: "", args: "", url: "", env: "" };
    adding = false;
  }

  /** `KEY=valor` por linha → objeto. Formato que o usuário já conhece de .env. */
  function parseEnv(raw: string): Record<string, string> {
    const out: Record<string, string> = {};
    for (const line of raw.split("\n")) {
      const eq = line.indexOf("=");
      if (eq <= 0) continue;
      out[line.slice(0, eq).trim()] = line.slice(eq + 1).trim();
    }
    return out;
  }

  async function run(fn: () => Promise<unknown>): Promise<void> {
    busy = true;
    error = "";
    try {
      await fn();
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function addServer(): Promise<void> {
    const server: McpServer = {
      name: form.name.trim(),
      command: form.command.trim(),
      args: form.args.trim() ? form.args.trim().split(/\s+/) : [],
      env: parseEnv(form.env),
      url: form.url.trim(),
      enabled: true,
    };
    await run(async () => {
      await api.mcpUpsert(tab, server);
      resetForm();
    });
  }

  async function installSkill(): Promise<void> {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    await run(() => api.skillInstall(tab, picked, projectRoot || undefined));
  }

  /** Para onde este servidor ainda pode ser copiado. */
  function copyTargets(name: string): HarnessId[] {
    return TABS.filter(
      (id) => id !== tab && !harnesses.find((h) => h.id === id)?.servers.some((s) => s.name === name),
    );
  }
</script>

<div class="backdrop" onclick={() => (app.harnessOpen = false)} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true">
    <div class="head">
      <h2>{t("harness.title")}</h2>
      <button class="x" onclick={() => (app.harnessOpen = false)} aria-label={t("confirm.close")}>
        <X size={16} />
      </button>
    </div>

    <div class="tabs">
      {#each TABS as id (id)}
        <button class="tab" class:on={tab === id} onclick={() => (tab = id)}>
          <span class="ic" style="color:{profile(id).color}"><ToolIcon {id} /></span>
          {profile(id).label}
        </button>
      {/each}
    </div>

    {#if error}
      <div class="err">{error}</div>
    {/if}

    {#if current}
      {#if !current.configured}
        <div class="note">{t("harness.notConfigured")}</div>
      {/if}
      {#if current.error}
        <div class="err">{current.error}</div>
      {/if}

      <div class="section">
        <div class="shead">
          <h3>{t("harness.mcpServers")}</h3>
          <button class="add" onclick={() => (adding = !adding)} disabled={busy}>
            <Plus size={14} />
            {t("harness.addServer")}
          </button>
        </div>
        <div class="path">{current.configPath}</div>

        {#if adding}
          <div class="form">
            <input placeholder={t("harness.fieldName")} bind:value={form.name} />
            <input placeholder={t("harness.fieldCommand")} bind:value={form.command} />
            <input placeholder={t("harness.fieldArgs")} bind:value={form.args} />
            <input placeholder={t("harness.fieldUrl")} bind:value={form.url} />
            <textarea placeholder={t("harness.fieldEnv")} rows="2" bind:value={form.env}></textarea>
            <div class="formbtns">
              <button class="cancel" onclick={resetForm}>{t("confirm.cancel")}</button>
              <button class="ok" onclick={addServer} disabled={busy || !form.name.trim()}>
                {t("harness.save")}
              </button>
            </div>
          </div>
        {/if}

        {#if current.servers.length === 0}
          <div class="empty">{t("harness.noServers")}</div>
        {:else}
          {#each current.servers as s (s.name)}
            <div class="srv" class:off={!s.enabled}>
              <input
                type="checkbox"
                checked={s.enabled}
                disabled={busy}
                onchange={(e) => run(() => api.mcpSetEnabled(tab, s.name, e.currentTarget.checked))}
                aria-label={s.name}
              />
              <div class="info">
                <div class="n">{s.name}</div>
                <div class="c">{s.url || [s.command, ...s.args].join(" ")}</div>
              </div>
              {#each copyTargets(s.name) as target (target)}
                <button
                  class="icon"
                  title={t("harness.copyTo").replace("{tool}", profile(target).label)}
                  disabled={busy}
                  onclick={() => run(() => api.mcpCopyTo(tab, target, s.name))}
                >
                  <Copy size={13} />
                  <span class="tiny">{profile(target).label}</span>
                </button>
              {/each}
              <button
                class="icon danger"
                title={t("confirm.delete")}
                disabled={busy}
                onclick={() => run(() => api.mcpRemove(tab, s.name))}
              >
                <Trash2 size={13} />
              </button>
            </div>
          {/each}
        {/if}
      </div>

      {#if tab === "claude"}
        <div class="section">
          <div class="shead">
            <h3>{t("harness.skills")}</h3>
            <button class="add" onclick={installSkill} disabled={busy}>
              <FolderOpen size={14} />
              {t("harness.installSkill")}
            </button>
          </div>
          <div class="path">{t("harness.skillsHint")}</div>

          {#if skills.length === 0}
            <div class="empty">{t("harness.noSkills")}</div>
          {:else}
            {#each skills as sk (sk.path)}
              <div class="srv">
                <div class="info">
                  <div class="n">
                    {sk.name}
                    {#if sk.projectScoped}<span class="badge">{t("harness.projectScope")}</span>{/if}
                    {#if sk.shared}
                      <span class="badge shared" title={t("harness.sharedSkillHint")}>
                        {t("harness.sharedSkill")}
                      </span>
                    {/if}
                  </div>
                  <div class="c">{sk.description || sk.path}</div>
                </div>
                <button
                  class="icon danger"
                  title={t("confirm.delete")}
                  disabled={busy}
                  onclick={() => run(() => api.skillRemove(tab, sk.path, projectRoot || undefined))}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            {/each}
          {/if}
        </div>
      {:else}
        <div class="section">
          <div class="empty">{t("harness.skillsClaudeOnly")}</div>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: var(--overlay);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .modal {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: 640px;
    max-width: 92vw;
    max-height: 84vh;
    overflow: auto;
    padding: 16px 18px 18px;
    box-shadow: 0 12px 40px var(--shadow);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    color: var(--fg);
  }
  h3 {
    margin: 0;
    font-size: 13px;
    color: var(--fg);
  }
  .x {
    background: none;
    border: none;
    color: var(--muted);
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
  }
  .x:hover {
    background: var(--elevated);
    color: var(--fg);
  }
  .tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 14px;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--elevated);
    border: 1px solid transparent;
    color: var(--muted);
    padding: 6px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
  }
  .tab.on {
    border-color: var(--accent);
    color: var(--fg);
  }
  .ic {
    width: 14px;
    height: 14px;
    display: block;
  }
  .section {
    margin-bottom: 18px;
  }
  .shead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .path {
    font-size: 11px;
    color: var(--muted);
    margin-bottom: 8px;
    word-break: break-all;
  }
  .add {
    display: flex;
    align-items: center;
    gap: 5px;
    background: var(--elevated);
    border: 1px solid var(--border);
    color: var(--fg);
    padding: 4px 9px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
  }
  .add:hover:not(:disabled) {
    background: var(--accent);
    color: var(--accent-fg);
    border-color: var(--accent);
  }
  .add:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .srv {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    margin-bottom: 6px;
  }
  .srv.off {
    opacity: 0.55;
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  .n {
    font-size: 12px;
    color: var(--fg);
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .c {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* Compartilhada: cor diferente da de projeto, porque o aviso é outro —
     "isto afeta outra ferramenta", não "isto é só deste repositório". */
  .badge.shared {
    background: color-mix(in srgb, var(--warning) 18%, transparent);
    border-color: color-mix(in srgb, var(--warning) 45%, transparent);
    color: var(--warning);
  }
  .badge {
    font-size: 10px;
    background: var(--elevated);
    color: var(--muted);
    padding: 1px 5px;
    border-radius: 4px;
  }
  .icon {
    display: flex;
    align-items: center;
    gap: 4px;
    background: none;
    border: 1px solid var(--border);
    color: var(--muted);
    padding: 3px 6px;
    border-radius: 5px;
    cursor: pointer;
  }
  .icon:hover:not(:disabled) {
    color: var(--fg);
    background: var(--elevated);
  }
  .icon.danger:hover:not(:disabled) {
    color: var(--danger);
    border-color: var(--danger);
  }
  .icon:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .tiny {
    font-size: 10px;
  }
  .empty,
  .note {
    font-size: 12px;
    color: var(--muted);
    padding: 8px 0;
  }
  .err {
    font-size: 12px;
    color: var(--danger);
    margin-bottom: 10px;
    word-break: break-word;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 10px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .form input,
  .form textarea {
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--fg);
    padding: 6px 8px;
    border-radius: 5px;
    outline: none;
    font-size: 12px;
    font-family: inherit;
  }
  .form input:focus,
  .form textarea:focus {
    border-color: var(--accent);
  }
  .formbtns {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .cancel,
  .ok {
    border: none;
    padding: 5px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
  }
  .cancel {
    background: var(--elevated);
    color: var(--fg);
  }
  .ok {
    background: var(--accent);
    color: var(--accent-fg);
  }
  .ok:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
