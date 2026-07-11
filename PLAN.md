# Perene v2 — Plano de reescrita multiplataforma (Tauri + Rust)

> **Como usar este plano:** abra uma sessão do Claude Code neste diretório e rode
> com o goal de executar os milestones M0→M6 em ordem. Cada milestone tem
> critérios de aceite — não avance sem fechá-los. Em caso de dúvida de design,
> as decisões da seção "Decisões travadas" prevalecem.

---

## 1. Contexto

O **Perene v1** é um app nativo de macOS (Swift/AppKit + SwiftTerm + tmux) que
gerencia múltiplos terminais rodando CLIs de IA (Claude Code, Codex, OpenCode)
e shell, organizados em workspaces → folders → abas → painéis, com persistência
de estado (fechar/reabrir o app mantém tudo; reiniciar a máquina retoma as
sessões de IA via resume nativo das CLIs). Ele vive em
`~/Projects/tool/zampimanager` e **continua em uso — não mexer nele**.

**Por que a v2:**
1. **Multiplataforma** — colegas de trabalho usam Windows/Linux.
2. **SwiftTerm tem limitações estruturais** — dead keys/acentos (marked text é
   stub), métodos `public` não-`open` (impossível interceptar keyDown/scroll),
   bugs de seleção. O xterm.js resolve tudo isso de graça.
3. **tmux não existe no Windows** — a persistência precisa de outra base.

**Requisito inegociável:** pouco uso de RAM. Alvo: **< 150 MB** com 5 terminais
abertos (medir e reportar a cada milestone). Electron está proibido; Tauri usa
a webview do sistema (WebView2 no Windows, WebKitGTK no Linux, WKWebView no Mac).

## 2. Decisões travadas (não re-litigar)

| Tema | Decisão | Motivo |
|---|---|---|
| Shell do app | **Tauri 2.x** | Webview do sistema, RAM baixa, dev rápido |
| Terminal | **xterm.js** (`@xterm/xterm` + addons fit, webgl, search, unicode11, clipboard) | Motor do VSCode; acentos/IME, seleção, scroll maduros |
| Frontend | **Svelte 5 + TypeScript + Vite** | Leve, sem virtual DOM, menos RAM que React |
| Editor de arquivos | **CodeMirror 6** | Monaco é pesado demais pro alvo de RAM |
| Persistência de sessões | **Daemon próprio em Rust** (substitui o tmux nas 3 plataformas) | Um caminho só; tmux não roda no Windows |
| PTY | crate **`portable-pty`** (wezterm) — ConPTY no Windows, forkpty no unix | Battle-tested |
| Lógica de negócio | **Crates Rust puros**, UI descartável | Migração futura pra UI nativa (iced/gpui) sem reescrever o core |
| Estado do app | `~/.perene2/` (unix) / `%APPDATA%\perene2\` (Windows) | Não colidir com `~/.perene/` da v1, que segue em uso |
| Claude resume | Criar com `--session-id <uuid>`, retomar com `--resume <uuid>`. **Nunca `--continue`** | Colide com múltiplas sessões na mesma pasta |
| Codex / OpenCode | `codex resume --last` / `opencode --continue`; histórico usa `resume <id>` / `--session <id>` | Igual v1 |
| Git | Nunca usar git worktrees; um branch novo por tarefa | Regra global do usuário |
| Idioma | UI e docs em pt-BR; código/identificadores em inglês | Padrão do usuário |

## 3. Arquitetura

```
perene-tauri/
├── Cargo.toml                 # workspace
├── crates/
│   ├── perene-core/           # modelos + persistência (Rust puro, zero deps de UI)
│   │   ├── manifest (v3): Workspace → Folders → Tabs → Panes
│   │   ├── LayoutNode (árvore binária de splits, por aba)
│   │   ├── IDs imutáveis: ws_/fold_/tab_/pane_/split_ (nunca reciclar)
│   │   └── escrita atômica (tmp+rename) + 1 backup .bak
│   ├── perene-daemon/         # servidor de sessões (processo separado, sobrevive à UI)
│   │   ├── 1 PTY por pane (portable-pty), com scrollback buffer em memória
│   │   ├── IPC: unix socket (mac/linux) / named pipe (Windows), protocolo JSON-lines
│   │   ├── attach/detach de clientes; replay de scrollback no reattach
│   │   ├── single-instance via lock no socket/pipe
│   │   └── flush de scrollback pra disco (~/.perene2/scrollback/) em shutdown limpo
│   └── perene-protocol/       # tipos do protocolo UI⇄daemon compartilhados
└── apps/
    └── desktop/               # Tauri 2 + Svelte 5 + xterm.js
        ├── src-tauri/         # comandos Tauri = cliente do daemon (spawn se não existir)
        └── src/               # UI: sidebar, bottom tab bar, grid de splits, xterm
```

**Fluxo principal:** UI cria pane → comando Tauri → daemon spawna PTY (login
shell: `$SHELL -l -c '<cmd>; exec $SHELL -l'` no unix; PowerShell no Windows)
→ output flui daemon→Tauri→xterm.js em chunks **com batching** (coalescer por
frame, ~16ms, senão IPC vira gargalo com output pesado) → input do xterm.js
volta pelo mesmo canal.

**Fechar a UI ≠ matar sessões:** o daemon segue vivo com os processos. Reabrir
a UI = reattach + replay de scrollback. Reiniciar a máquina = daemon morre,
sessões de IA retomam via flags de resume das CLIs (igual v1).

## 4. Milestones

### M0 — Esqueleto + terminal funcional (validação de risco)
- Workspace Rust + Tauri 2 + Svelte 5 + xterm.js com **um** terminal local
  (PTY direto no processo Tauri, ainda sem daemon).
- Login shell com PATH correto (claude/codex/opencode acháveis).
- **Aceite (testar manualmente e reportar):** acentos via dead keys (option+e →
  é), shift+enter quebra linha no Claude Code, scroll com trackpad, seleção +
  cmd/ctrl+C copia, colar funciona, cores 256/RGB ok, `vim` utilizável.
- Criar `CLAUDE.md` do projeto (curto: regras deste plano) + `git init` + primeiro commit.
- **Este milestone é o gate do projeto**: se algo do aceite falhar de forma
  incontornável, PARAR e reportar antes de seguir.

### M1 — Daemon de sessões
- Extrair os PTYs pro `perene-daemon`; UI vira cliente (spawn automático do
  daemon se não estiver rodando; adoption se já estiver).
- Attach/detach/reattach com replay de scrollback (mín. 10k linhas).
- **Aceite:** fechar a janela, reabrir → mesmo shell, mesmo processo vivo
  (validar com `sleep 300` rodando), scrollback preservado. Teste automatizado
  do protocolo (sem UI).

### M2 — Modelo de dados + persistência
- Portar o manifest da v1 (referência: `~/Projects/tool/zampimanager/Sources/Perene/Models.swift`
  e `docs/data-model.md`) pro `perene-core` como manifest v3.
- Save síncrono em mudança estrutural; debounce só pra cwd e ratio de split.
- Restore lazy: só a aba ativa atacha PTYs; o resto é pendingRestore.
- **Aceite:** testes de round-trip do manifest; testes NUNCA tocam o diretório
  real (injetar diretório — lição da v1, onde `swift test` apagou o manifest
  do usuário).

### M3 — UI completa
- Sidebar (workspaces/folders/abas, rename, drag&drop), bottom tab bar,
  grid de splits (presets de layout), diretório por workspace/folder.
- Perfis de ferramenta: claude / codex / opencode / shell, com ícone e cor.
- Flag YOLO nas configurações (claude `--dangerously-skip-permissions`,
  codex `--dangerously-bypass-approvals-and-sandbox`, opencode `--auto`).
- Cmd/Ctrl+V de imagem → salva em `~/.perene2/paste/` e cola o path.
- **Aceite:** paridade de fluxo com a v1 no uso diário; atalhos documentados.

### M4 — Resume + histórico + usage
- Resume pós-reboot (detectar daemon morto → recriar PTYs com flags de resume).
- Histórico de sessões dos 3 harnesses com filtro (referência:
  `SessionHistoryController.swift` da v1).
- Painel de Usage/tokens (referência: `UsageProvider.swift` da v1 — cache em
  disco, alvo < 10s frio / < 1s quente).
- **Aceite:** reboot simulado (matar daemon) → abas voltam e CLIs retomam a
  MESMA conversa.

### M5 — Git + visualizador de arquivos
- Pane de arquivos estilo mini-VSCode: árvore com cores de status git,
  Files|Changes, diff viewer, editor CodeMirror 6 (numeração de linhas,
  busca/substituição, ⌘S salva), syntax highlight tema Dark+.
- Barra git no topo: branch, ahead/behind, dirty, checkout/criar branch,
  fetch/pull, abrir PR no browser (gh).
- **Aceite:** abrir/editar/salvar arquivo; ver diff de arquivo modificado.

### M6 — Packaging + CI
- Tauri bundler: `.dmg` (mac), `.msi`/NSIS (Windows), `.deb`/AppImage (Linux).
- GitHub Actions com matrix de build nas 3 plataformas (mínimo: compilar +
  testes; ideal: artefatos de release).
- Medição final de RAM com 5 terminais (reportar por plataforma disponível).
- **Aceite:** instalador de mac gerado e testado localmente; CI verde.

## 5. Lições da v1 (obrigatório respeitar)

1. **Testes jamais escrevem no estado real** — injeção de diretório em tudo
   que persiste. (Na v1, `swift test` sobrescreveu o manifest do usuário 2x.)
2. **Single-instance** — duas UIs ok (multi-window futuro), dois daemons NUNCA.
3. **Escrita atômica + .bak** em todo arquivo de estado.
4. **IDs imutáveis, nunca reciclar.**
5. **Batching de output** — output pesado (builds, logs) não pode travar a UI
   nem inflar o IPC; coalescer por frame.
6. **Login shell sempre** — senão as CLIs não estão no PATH. No Windows,
   resolver PATH do usuário (PowerShell profile).
7. **Save síncrono em mudança estrutural** — crash não pode perder abas.
8. **`claude --continue` é proibido** — só `--session-id`/`--resume`.

## 6. Riscos conhecidos

- **WebKitGTK (Linux)** é a webview mais fraca — testar xterm.js cedo; addon
  webgl pode precisar de fallback pra canvas/dom.
- **Throughput do IPC Tauri** — se eventos Tauri não derem conta de output
  bruto de terminal, alternativa: daemon expõe WebSocket em localhost e o
  xterm.js conecta direto (attach addon).
- **ConPTY** tem peculiaridades (resize, ANSI) — testar no Windows o quanto antes.
- **RAM da webview** varia por plataforma — medir desde M0, não só no fim.

## 7. Fora de escopo da v2.0

- Colaboração/sync entre máquinas; temas customizáveis; plugins; mobile.
- Migração automática de estado da v1 → v2 (fazer manualmente depois, se valer).
