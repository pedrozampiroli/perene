# Perene v2 — instruções do projeto (Claude Code)

Reescrita multiplataforma do Perene (gerenciador de terminais para CLIs de IA).
O plano completo está em `PLAN.md`; as regras abaixo são o resumo operacional.

## Regras invioláveis (lições da v1)

1. **Testes JAMAIS escrevem no estado real.** Todo código que persiste recebe o
   diretório por injeção; testes usam `tempfile`. (Na v1, `swift test` apagou o
   manifest do usuário 2x.)
2. **Single-instance do daemon.** Duas UIs, ok; dois daemons, NUNCA.
3. **Escrita atômica + `.bak`** em todo arquivo de estado (tmp + rename, 1 backup).
4. **IDs imutáveis, nunca reciclar** (`ws_`/`fold_`/`tab_`/`pane_`/`split_`).
5. **Batching de output** — coalescer PTY→UI por frame (~8-16ms); output pesado
   não pode inflar o IPC nem travar a UI.
6. **Login shell sempre** — senão as CLIs (claude/codex/opencode) não estão no
   PATH. No Windows, PowerShell carrega o profile do usuário.
7. **Save síncrono em mudança estrutural.** Debounce só para cwd e ratio de split.
8. **`claude --continue` é PROIBIDO.** Resume só via `--session-id`/`--resume`.

## Arquitetura

- `crates/perene-core` — modelos + persistência (Rust puro, zero deps de UI).
- `crates/perene-protocol` — tipos do protocolo UI ⇄ daemon (serde, `camelCase`
  no wire para casar com o front sem conversão).
- `crates/perene-daemon` — servidor de sessões (1 PTY/pane, sobrevive à UI).
- `apps/desktop` — Tauri 2 + Svelte 5 + xterm.js. `src-tauri` é o backend;
  `src` é a UI.

Estado do app: `~/.perene2/` (unix) / `%APPDATA%\perene2\` (Windows). Nunca
colidir com `~/.perene/` da v1.

## Convenções

- **Git:** nunca usar worktrees; um branch novo por tarefa. Não commitar sem
  pedido explícito.
- **Idioma:** UI e docs em pt-BR; código e identificadores em inglês.
- **RAM:** alvo < 150 MB com 5 terminais. Medir a cada milestone.
- **Comandos:**
  - `cargo build` / `cargo test` na raiz (workspace).
  - `cd apps/desktop && npm run tauri dev` para rodar o app.
  - `cd apps/desktop && npm run build` para o bundle do front.

## Referência da v1

Código Swift da v1 em `~/Projects/tool/zampimanager` (em uso — **não mexer**).
Arquivos úteis: `Sources/Perene/Models.swift`, `docs/data-model.md`,
`SessionHistoryController.swift`, `UsageProvider.swift`, `GitController.swift`.
