<script lang="ts">
  import { onMount } from "svelte";
  import { X } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";
  import { api } from "../lib/api";
  import { t, LOCALES } from "../lib/i18n.svelte";
  import type { ShellOption } from "../lib/types";

  let shells = $state<ShellOption[]>([]);
  onMount(async () => {
    try {
      shells = await api.listShells();
    } catch {
      shells = [];
    }
  });

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
  }
  h2 {
    font-size: 16px;
    margin: 0 0 8px;
  }
  h3 {
    font-size: 12px;
    text-transform: uppercase;
    color: #8a8a8a;
    margin: 16px 0 8px;
  }
  .close {
    background: none;
    border: none;
    color: #888;
    font-size: 14px;
    cursor: pointer;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid #333;
  }
  .t {
    font-size: 14px;
  }
  .sub {
    font-size: 12px;
    color: #8a8a8a;
    margin-top: 2px;
  }
  .stepper {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  select {
    max-width: 260px;
    background: #1e1e1e;
    border: 1px solid #3a3a3a;
    color: #fff;
    padding: 6px 8px;
    border-radius: 6px;
    font-size: 12px;
    outline: none;
  }
  .tour {
    background: #3a3d41;
    border: none;
    color: #ddd;
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
    white-space: nowrap;
  }
  .tour:hover {
    background: #4a4d51;
    color: #fff;
  }
  .stepper button {
    background: #3a3d41;
    border: none;
    color: #fff;
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
    background: #1e1e1e;
    border: 1px solid #444;
    border-radius: 4px;
    padding: 1px 6px;
    font-family: monospace;
    font-size: 12px;
    text-align: center;
  }
</style>
