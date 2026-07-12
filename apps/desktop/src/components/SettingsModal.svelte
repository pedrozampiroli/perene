<script lang="ts">
  import { X } from "@lucide/svelte";
  import { app } from "../lib/store.svelte";

  const shortcuts: [string, string][] = [
    ["⌘T", "Novo terminal (shell)"],
    ["⌘W", "Fechar painel ativo"],
    ["⌘D", "Dividir à direita"],
    ["⌘⇧D", "Dividir abaixo"],
    ["⌘1–9", "Ir para a aba N"],
    ["⌘,", "Configurações"],
    ["⌘C / ⌃⇧C", "Copiar seleção"],
    ["⌘V / ⌃⇧V", "Colar (texto ou imagem)"],
    ["⇧Enter", "Nova linha (Claude Code)"],
  ];
</script>

<div class="backdrop" onclick={() => (app.settingsOpen = false)} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true">
    <div class="head">
      <h2>Configurações</h2>
      <button class="close" onclick={() => (app.settingsOpen = false)}><X size={16} /></button>
    </div>

    <label class="row">
      <div>
        <div class="t">Modo YOLO</div>
        <div class="sub">Pula permissões das CLIs (claude/codex/opencode). Cuidado.</div>
      </div>
      <input type="checkbox" checked={app.settings.yolo} onchange={() => app.toggleYolo()} />
    </label>

    <label class="row">
      <div>
        <div class="t">Renderizador WebGL</div>
        <div class="sub">Mais rápido, porém MUITO mais RAM. Desligado por padrão.</div>
      </div>
      <input type="checkbox" checked={app.settings.webgl} onchange={() => app.toggleWebgl()} />
    </label>

    <div class="row">
      <div>
        <div class="t">Tamanho da fonte</div>
        <div class="sub">Aplicado a novos terminais.</div>
      </div>
      <div class="stepper">
        <button onclick={() => app.setFontSize(app.settings.fontSize - 1)}>−</button>
        <span>{app.settings.fontSize}</span>
        <button onclick={() => app.setFontSize(app.settings.fontSize + 1)}>+</button>
      </div>
    </div>

    <div class="shortcuts">
      <h3>Atalhos</h3>
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
