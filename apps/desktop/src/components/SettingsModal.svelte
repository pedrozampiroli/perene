<script lang="ts">
  import { onMount } from "svelte";
  import { X } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { t, LOCALES } from "../lib/i18n.svelte";
  import { theme } from "../lib/theme.svelte";
  import type { ShellOption } from "../lib/types";
  import { open } from "@tauri-apps/plugin-dialog";

  let shells = $state<ShellOption[]>([]);
  let themeError = $state("");
  onMount(async () => {
    try {
      shells = await api.listShells();
    } catch {
      shells = [];
    }
  });

  /** Importa uma família de temas do Zed e já ativa o primeiro dela. */
  async function importZedTheme(): Promise<void> {
    themeError = "";
    const picked = await open({
      multiple: false,
      filters: [{ name: "Zed theme", extensions: ["json"] }],
    });
    if (typeof picked !== "string") return;
    try {
      const imported = await api.themeImportZed(picked);
      await theme.refreshCatalog();
      if (imported.length > 0) await app.setTheme(imported[0].id);
    } catch (e) {
      themeError = String(e);
    }
  }

  const shortcuts = $derived<[string, string][]>([
    ["⌘T", t("shortcuts.newTerminal")],
    ["⌘W", t("shortcuts.closePane")],
    ["⌘D", t("shortcuts.splitRight")],
    ["⌘⇧D", t("shortcuts.splitDown")],
    ["⌘1–9", t("shortcuts.goToTab")],
    ["⌘,", t("shortcuts.settings")],
    ["⌘Y", t("shortcuts.history")],
    ["⌘U", t("shortcuts.usage")],
    ["⌘C / ⌃⇧C", t("shortcuts.copy")],
    ["⌘V / ⌃⇧V", t("shortcuts.paste")],
    ["⇧Enter", t("shortcuts.newline")],
  ]);
</script>

<div class="backdrop" onclick={() => (app.settingsOpen = false)} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true">
    <div class="head">
      <h2>{t("settings.title")}</h2>
      <button class="close" onclick={() => (app.settingsOpen = false)}><X size={16} /></button>
    </div>

    <label class="row">
      <div>
        <div class="t">{t("settings.language")}</div>
        <div class="sub">{t("settings.languageHint")}</div>
      </div>
      <select value={app.settings.locale} onchange={(e) => app.setLocale(e.currentTarget.value)}>
        <option value="">Auto</option>
        {#each LOCALES as l (l.code)}
          <option value={l.code}>{l.flag} {l.name}</option>
        {/each}
      </select>
    </label>

    <div class="row">
      <div>
        <div class="t">{t("settings.theme")}</div>
        <div class="sub">{t("settings.themeHint")}</div>
      </div>
      <div class="themepick">
        <select
          value={app.settings.theme || "dark-plus"}
          onchange={(e) => app.setTheme(e.currentTarget.value)}
        >
          {#each theme.available as th (th.id)}
            <option value={th.id}>{th.name}</option>
          {/each}
        </select>
        <button class="tour" onclick={importZedTheme}>{t("settings.themeImport")}</button>
      </div>
    </div>
    {#if themeError}
      <div class="row err">{themeError}</div>
    {/if}

    <div class="row">
      <div>
        <div class="t">{t("settings.harness")}</div>
        <div class="sub">{t("settings.harnessHint")}</div>
      </div>
      <button
        class="tour"
        onclick={() => {
          app.settingsOpen = false;
          app.harnessOpen = true;
        }}>{t("settings.harnessBtn")}</button
      >
    </div>

    <label class="row">
      <div>
        <div class="t">{t("settings.yolo")}</div>
        <div class="sub">{t("settings.yoloHint")}</div>
      </div>
      <input type="checkbox" checked={app.settings.yolo} onchange={() => app.toggleYolo()} />
    </label>

    <label class="row">
      <div>
        <div class="t">{t("settings.askWorktree")}</div>
        <div class="sub">{t("settings.askWorktreeHint")}</div>
      </div>
      <input type="checkbox" checked={app.settings.askWorktree} onchange={(e) => app.setAskWorktree(e.currentTarget.checked)} />
    </label>

    <label class="row">
      <div>
        <div class="t">{t("settings.acpMode")}</div>
        <div class="sub">{t("settings.acpModeHint")}</div>
      </div>
      <input type="checkbox" checked={app.settings.acpMode} onchange={(e) => app.setAcpMode(e.currentTarget.checked)} />
    </label>

    {#if app.settings.acpMode}
      <label class="row">
        <div>
          <div class="t">{t("settings.acpTerminal")}</div>
          <div class="sub">{t("settings.acpTerminalHint")}</div>
        </div>
        <input type="checkbox" checked={app.settings.acpTerminal} onchange={(e) => app.setAcpTerminal(e.currentTarget.checked)} />
      </label>
    {/if}

    <label class="row">
      <div>
        <div class="t">{t("settings.webgl")}</div>
        <div class="sub">{t("settings.webglHint")}</div>
      </div>
      <input type="checkbox" checked={app.settings.webgl} onchange={() => app.toggleWebgl()} />
    </label>

    <div class="row">
      <div>
        <div class="t">{t("settings.fontSize")}</div>
        <div class="sub">{t("settings.fontSizeHint")}</div>
      </div>
      <div class="stepper">
        <button onclick={() => app.setFontSize(app.settings.fontSize - 1)}>−</button>
        <span>{app.settings.fontSize}</span>
        <button onclick={() => app.setFontSize(app.settings.fontSize + 1)}>+</button>
      </div>
    </div>

    <div class="row">
      <div>
        <div class="t">{t("settings.shell")}</div>
        <div class="sub">{t("settings.shellHint")}</div>
      </div>
      <select value={app.settings.shell} onchange={(e) => app.setShell(e.currentTarget.value)}>
        <option value="">{t("settings.shellDefault")}</option>
        {#each shells as s (s.path)}
          <option value={s.path}>{s.label} — {s.path}</option>
        {/each}
      </select>
    </div>

    <div class="row">
      <div>
        <div class="t">{t("onb.replay")}</div>
        <div class="sub">{t("onb.replayHint")}</div>
      </div>
      <button class="tour" onclick={() => { app.settingsOpen = false; app.openOnboarding(); }}>
        {t("onb.replayBtn")}
      </button>
    </div>

    <div class="shortcuts">
      <h3>{t("settings.shortcuts")}</h3>
      {#each shortcuts as [k, d] (k)}
        <div class="sc"><kbd>{k}</kbd><span>{d}</span></div>
      {/each}
    </div>
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
    width: 440px;
    max-height: 80vh;
    overflow-y: auto;
    background: var(--panel);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px 20px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  h2 {
    font-size: 16px;
    margin: 0 0 8px;
  }
  h3 {
    font-size: 12px;
    text-transform: uppercase;
    color: var(--muted);
    margin: 16px 0 8px;
  }
  .close {
    background: none;
    border: none;
    color: var(--muted);
    font-size: 14px;
    cursor: pointer;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }
  .t {
    font-size: 14px;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
    margin-top: 2px;
  }
  .stepper {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  select {
    max-width: 260px;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--fg);
    padding: 6px 8px;
    border-radius: 6px;
    font-size: 12px;
    outline: none;
  }
  .tour {
    background: var(--elevated);
    border: none;
    color: var(--fg);
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
    white-space: nowrap;
  }
  .tour:hover {
    background: var(--border);
    color: var(--fg);
  }
  .stepper button {
    background: var(--elevated);
    border: none;
    color: var(--fg);
    width: 24px;
    height: 24px;
    border-radius: 4px;
    cursor: pointer;
  }
  .sc {
    display: flex;
    gap: 12px;
    padding: 3px 0;
    font-size: 13px;
  }
  kbd {
    display: inline-block;
    min-width: 60px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 6px;
    font-family: monospace;
    font-size: 12px;
    text-align: center;
  }
  .themepick {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .err {
    color: var(--danger);
    font-size: 12px;
  }
</style>
