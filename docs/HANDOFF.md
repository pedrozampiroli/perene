# Handoff — infra do Perene v2

> **Leia isto primeiro ao retomar o projeto.** Complementa o `PLAN.md` (plano
> original dos milestones), o `CLAUDE.md` (regras operacionais) e o `git log`.
> Atualizado em 2026-08-04.

## Estado atual

O `PLAN.md` (M0–M6) foi concluído. Depois disso vieram várias rodadas de uso
real, e o app é usado no dia a dia.

- **Repositório:** https://github.com/pedrozampiroli/perene — **público**, MIT.
- **CI verde nas 3 plataformas** (macOS · Ubuntu · Windows) a cada push, mais
  CodeQL. Tudo vai direto no `main`; há um colaborador (`joao-tegasistemas`),
  que contribuiu por PR.
- **Instalado** em `/Applications/Perene.app` (macOS). Build local:
  `.dmg` sai em `target/release/bundle/dmg/`.
- **RAM:** ~109 MB com 5 terminais (alvo do plano: < 150 MB).

## Como rodar, testar e instalar

```bash
cd apps/desktop
npm install
npm run tauri dev       # app com hot reload (vite na porta 5273)
npm run check           # typecheck do front (svelte-check) — precisa ficar 0 erros
npm run tauri build     # gera .app + instalador

cargo test --workspace  # testes Rust (rodam nas 3 plataformas no CI)
```

**Instalar o build no /Applications** (macOS) — mate só a UI, nunca o daemon:

```bash
# 1. fecha SÓ a janela (o daemon segue vivo com as sessões do usuário)
ps -Ao pid=,args= | grep -a "Contents/MacOS/perene-desktop" \
  | grep -av "grep\|--daemon" | awk '{print $1}' | xargs -r kill -9
# 2. instala e relança
rsync -a --delete target/release/bundle/macos/Perene.app/ /Applications/Perene.app/
xattr -dr com.apple.quarantine /Applications/Perene.app
codesign --force --deep -s - /Applications/Perene.app
open /Applications/Perene.app
```

Mudança só de frontend/Tauri → o daemon antigo continua servindo e as sessões
sobrevivem. Mudança no **daemon** (`pty.rs`/`session.rs`/`server.rs`/`winpipe.rs`)
só vale com daemon novo — **peça permissão antes**, porque reiniciá-lo encerra
as sessões abertas.

## Arquitetura

```
crates/
  perene-protocol/   tipos IPC (ClientMessage/DaemonMessage) + framing JSON-lines
  perene-core/       models (manifest v3) · store (atômico) · settings · paths
                     history · usage · sqlite   — Rust puro, sem deps de UI
  perene-daemon/     session (1 PTY/pane + scrollback) · server (unix socket /
                     named pipe, single-instance) · pty (login shell) · winpipe
apps/desktop/
  src-tauri/src/     lib.rs (registra os comandos) · main.rs (--daemon reexec)
                     client.rs (cliente do daemon) · state.rs (manifest/settings/
                     history/usage/paste) · files.rs (fs + git + busca) · shells.rs
  src/lib/           store.svelte.ts (estado + ações) · i18n.svelte.ts · api.ts
                     terminal.ts (xterm) · editor.ts (CodeMirror) · profiles.ts
                     types.ts · paths.ts
  src/i18n/          en.json (fonte da verdade) · pt-BR.json · es.json
  src/components/    App · Sidebar · TopBar · BottomBar · TabGrid · SplitContainer
                     PaneView · FilesPane · FileTree · GitWidget · ToolIcon
                     SettingsModal · HistoryModal · UsageModal · NameModal
                     ConfirmModal · NewSessionModal · ContextMenu · SearchPalette
                     Onboarding
```

**A UI é descartável de propósito**: a lógica vive nos crates Rust puros, então
dá pra trocar o front por um nativo sem reescrever o core.

**App e daemon são o MESMO binário.** A UI reexecuta a si mesma com `--daemon`
(`client.rs::daemon_command()`), então não há sidecar pra empacotar. O daemon
sobrevive à janela; ao reabrir, a UI conecta no endpoint existente e adota o
daemon vivo (as sessões voltam com replay de scrollback).

Estado do app em `~/.perene2/` (`%APPDATA%\perene2\` no Windows):
`manifest.json` (+`.bak`), `settings.json`, `daemon.sock`/named pipe,
`daemon.lock`, `scrollback/`, `paste/`, `usage-cache.json`.

### Fluxos que vale conhecer

- **Spawn de terminal:** UI → `terminal_spawn` → daemon abre PTY com login shell
  (`$SHELL -l`, ou o shell escolhido nas settings) → output coalescido por frame
  (~8 ms) → evento Tauri → xterm.
- **Resume:** `profiles.ts::buildCommand(pane, settings, isFresh)` decide entre
  criar (`claude --session-id`), retomar pós-reboot (`--resume`/`resume --last`/
  `--continue`) ou retomar do histórico. Todo resume tem `|| <fresh>` como
  fallback pra nunca deixar o usuário num erro. **Nunca usar `claude --continue`.**
- **Worktree por sessão:** ao abrir sessão num repo, o modal oferece raiz do
  projeto · worktree existente · worktree nova. A nova nasce em
  `.perene/worktrees/<nome>` e o `.perene/` entra no `.gitignore` automaticamente
  (`create_project_worktree`).
- **i18n:** `t("chave")` em tudo; `import.meta.glob` carrega os JSONs, então
  **idioma novo = duplicar `en.json`** e traduzir. Chave faltando cai no inglês.

## Funcionalidades (mapa)

| Área | O que existe |
|---|---|
| Terminais | 1 PTY/pane no daemon, scrollback, reattach, login shell, shell configurável, dead keys, Shift+Enter (CSI u), paste de imagem |
| Organização | workspaces → pastas → abas → splits; drag & drop; presets de layout; painéis redimensionáveis (largura persistida) |
| Sessões | perfis claude/codex/opencode/shell, YOLO, resume pós-reboot, histórico com preview, worktree isolada |
| Editor | multi-abas (undo/cursor por arquivo), ⌘S, syntax highlight, diff lado a lado, árvore com status git |
| Git | branch/ahead/behind no topo, switch/create branch, fetch/pull/push, PR via `gh`, commits (log + `git show`), worktrees, commit box |
| Busca | ⌘P quick open (fuzzy), ⌘⇧F busca global (ripgrep), ⌘⇧H substituição global, ⌘F/⌘H no arquivo |
| UX | menus de contexto, modais de nomeação, confirmação antes de excluir/fechar, onboarding com spotlight, i18n (en/pt-BR/es), usage de tokens |

## Convenções (não quebrar)

1. **Nenhuma string solta na UI.** Use `t("chave")` e adicione em `en.json`
   (fonte da verdade) **e** nas traduções. Docs e comentários seguem em pt-BR.
2. **Testes nunca tocam o estado real** — `PERENE2_STATE_DIR` + injeção de
   caminho. (Na v1, `swift test` apagou o manifest do usuário 2x.)
3. **Nunca usar git worktrees para o próprio desenvolvimento** (regra global do
   usuário); um branch por tarefa. As worktrees do produto são outra coisa.
4. **Escrita atômica + `.bak`** em todo arquivo de estado.
5. **IDs imutáveis, nunca reciclados** (`ws_`/`fold_`/`tab_`/`pane_`/`split_`).
6. **Save síncrono em mudança estrutural**; debounce só pra cwd/ratio/larguras.

## Gotchas (erros que já custaram tempo)

1. **serde `rename_all` em ENUM só renomeia VARIANTES, não os campos.**
   `LayoutNode::Leaf { pane_id }` ia como `pane_id` (snake) e o front lia
   `paneId=undefined` → terminal nunca spawnava. Fix: `#[serde(rename_all)]` em
   CADA variante. Tem teste de regressão.
2. **RAM: o renderer WebGL do xterm é o vilão.** Com 5 terminais o WebContent
   ia a 274 MB (~360 MB total). Virou opção nas Configurações, **default OFF**
   → 109 MB. Se ligar WebGL, conta com o custo.
3. **App = daemon no MESMO binário** (`perene-desktop --daemon`), sem sidecar.
   `PERENE_DAEMON_BIN` permite apontar um binário standalone (testes/dev).
4. **Medir RAM: rodar o `.app` via `open`**, não o binário cru em background
   (binário backgrounded morre por SIGHUP quando o shell sai). Use
   `vmmap --summary <pid>` (phys_footprint), não RSS (superconta páginas
   compartilhadas). Atribua os helpers WebKit por PID > pid-do-app (ou kill-diff).
5. **Existem DOIS apps chamados "Perene"**: a v1 (Swift, `~/Projects/tool/
   zampimanager`, em uso pelo usuário) e esta v2. Cuidado ao dar `pkill`/
   screenshot — filtre por `perene-desktop`.
6. **Testes NUNCA tocam estado real**: `PERENE2_STATE_DIR` (env) redireciona
   tudo; stores recebem o caminho por injeção. Mantenha isso.
7. **Teste cross-platform ou o Windows apodrece calado.** `tests/protocol.rs`
   era `#![cfg(unix)]`, então o CI ficava verde no Windows testando NADA do IPC
   — e o app lá abria terminais que não recebiam tecla. Hoje o teste roda nas 3
   plataformas (módulo `platform` no topo do arquivo isola shell/endpoint).
   Duas armadilhas do named pipe que motivam o código como está:
   - **I/O sobreposto é obrigatório.** Handle síncrono serializa as operações no
     mesmo file object: a leitura bloqueada segurando o socket travaria a
     escrita do output dos PTYs (= terminal mudo).
   - **Uma instância do pipe atende UM cliente**: o `accept` cria a próxima
     instância a cada conexão.
   Single-instance no Windows é abertura com `share_mode(0)` no lockfile (o
   `flock` do unix), e o daemon sobe com `DETACHED_PROCESS` (equivalente ao
   `setsid`, e sem janelinha preta piscando).
8. **O ambiente do harness VAZA para os PTYs e quebra o resume.** Abrir o Perene
   de dentro de uma sessão do Claude Code fazia os terminais herdarem
   `CLAUDE_CODE_CHILD_SESSION` / `CLAUDE_CODE_SESSION_ID` / `CLAUDECODE`; o
   `claude` do pane então dizia *"Transcript saving is off"* e **nunca gravava a
   conversa** — por isso o `--resume` falhava depois com *"No conversation found
   with session ID"*. `pty.rs::sanitize_env()` limpa tudo com prefixo
   `CLAUDE_CODE_` (+ equivalentes) antes de spawnar. Teste:
   `spawned_terminals_do_not_inherit_harness_session_env`.
9. **`dragDropEnabled: false` é obrigatório no Tauri.** Com o default (`true`),
   o handler NATIVO de arrastar-soltar da janela engole os eventos antes da
   webview: o `dragstart` funciona (é JS puro) mas o **`drop` nunca dispara**, e
   arrastar aba pra pasta parece "não aceitar". Está em `tauri.conf.json`.
10. **Scrollbar mudando a largura do painel.** Sem `scrollbar-gutter: stable`,
    abrir uma aba com lista longa (Commits) faz a barra aparecer e **cortar** o
    conteúdo. As abas do editor também são ícones de largura fixa justamente pra
    o layout não pular ao alternar.
11. **Nunca dar `pkill` genérico em "perene-desktop"** — pega o daemon junto e
    **destrói as sessões vivas do usuário**. Filtre por `--daemon` para excluí-lo.
12. **Antes de publicar qualquer coisa** (repo público, README, screenshot):
    varra segredos e **olhe o screenshot**. Uma captura do app pegou repositório
    privado da empresa, PR interna e URLs de CI — foi descartada.

## Limitações conhecidas / próximos passos

- **Windows**: funcional desde 2026-07-28 — o IPC do daemon virou named pipe
  (`crates/perene-daemon/src/winpipe.rs`). Antes disso `run`/`connect` eram
  stubs que retornavam erro: o app abria o pane, o `terminal_spawn` falhava em
  silêncio e **não dava para digitar em terminal nenhum**. Detalhes em
  "Gotchas" #7.
- **Linux**: compila no CI e o IPC é o mesmo do mac (unix socket), mas nunca foi
  rodado de verdade. WebKitGTK pode precisar de fallback do renderer.
- **cwd tracking** (atualizar `pane.workingDirectory` via OSC 7) não foi
  implementado — o cwd fica o do spawn. Debounce de save já está pronto pra isso.
- **Busca global no Windows sem ripgrep**: `search_in_files` usa `rg` e, sem ele,
  cai no `grep -rn` — que não existe no Windows. Sem `rg` instalado, o ⌘⇧F
  devolve vazio (não quebra, só não acha nada). Falta um fallback nativo.
- **`bundle_dmg.sh` falha às vezes** no `npm run tauri build` (o `.app` sai
  normal, só o `.dmg` não). Não investigado — bloqueia gerar release com
  instalador.
- **Fila sugerida**: (1) cwd tracking (OSC 7); (2) fallback nativo da busca
  global; (3) resolver o `.dmg` e gerar release v0.1.0 com artefatos (o job
  `bundle` roda em tags `v*` ou dispatch manual).

## Referências da v1 (não mexer — `~/Projects/tool/zampimanager`)

`Sources/Perene/Models.swift`, `docs/data-model.md`, `SessionHistory.swift`,
`UsageProvider.swift`, `GitController.swift`, `SessionHistoryController.swift`.
Os SVGs das marcas em `icons/` vieram de lá.
