# Perene v2

Gerenciador multiplataforma de terminais para CLIs de IA (Claude Code, Codex,
OpenCode) e shell, organizados em **workspaces → folders → abas → painéis**, com
persistência de sessões que sobrevive ao fechamento da janela e ao reboot.

Reescrita da v1 (macOS nativo, Swift/AppKit + SwiftTerm + tmux) em **Tauri 2 +
Rust + Svelte 5 + xterm.js**, para rodar em macOS, Windows e Linux com baixo uso
de RAM (webview do sistema; Electron proibido).

## Arquitetura

```
crates/
  perene-protocol/  tipos do protocolo UI ⇄ daemon (JSON-lines)
  perene-core/      manifest v3 + persistência atômica, histórico, usage, git
  perene-daemon/    servidor de sessões: 1 PTY/pane, sobrevive à UI
apps/desktop/
  src-tauri/        backend Tauri (cliente do daemon); o mesmo binário vira o
                    daemon quando executado com `--daemon` (sem sidecar)
  src/              UI Svelte 5 + xterm.js + CodeMirror 6
```

- **Terminal:** xterm.js (acentos/IME, seleção, scroll, truecolor maduros).
- **Sessões:** um daemon próprio (substitui o tmux nas 3 plataformas) mantém os
  PTYs vivos; fechar a janela não mata as sessões — reabrir faz reattach com
  replay de scrollback. Reboot: as CLIs retomam via flags de resume.
- **Estado:** `~/.perene2/` (unix) / `%APPDATA%\perene2\` (Windows). Escrita
  atômica (tmp+rename) + backup `.bak`.

## Desenvolvimento

Requisitos: Rust, Node 22+, e (Linux) as libs do WebKitGTK.

```bash
cd apps/desktop
npm install
npm run tauri dev      # roda o app
npm run tauri build    # gera o instalador (.dmg / .msi / .deb / AppImage)
```

Testes (workspace Rust): `cargo test --workspace`.
Typecheck do front: `cd apps/desktop && npm run check`.

## Atalhos

Ver [`docs/atalhos.md`](docs/atalhos.md) ou o diálogo de Configurações (⌘,).

## Status por plataforma

- **macOS:** completo (desenvolvido e testado aqui).
- **Windows/Linux:** compilam no CI. O transporte IPC do daemon (named pipes no
  Windows) ainda é stub — runtime completo pendente. WebKitGTK (Linux) pode
  precisar de fallback do renderer webgl do xterm para canvas/DOM.

## Licença

MIT.