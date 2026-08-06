# Handoff — infra do Perene v2

> **Leia isto primeiro ao retomar o projeto.** Complementa o `PLAN.md` (plano
> original dos milestones), o `CLAUDE.md` (regras operacionais) e o `git log`.
> Atualizado em 2026-08-06.

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
- **Modo ACP** (chat estruturado em vez da CLI no terminal) implementado no
  branch `feat/acp-mode`, desligado por padrão — ver a seção "Modo ACP".

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
  perene-acp/        cliente do Agent Client Protocol: jsonrpc (genérico sobre
                     Read/Write) · protocol (tipos do wire) · agent (spawn + API)
  perene-daemon/     session (1 PTY/pane + scrollback) · server (unix socket /
                     named pipe, single-instance) · pty (login shell) · winpipe
                     acp (sessões ACP + transcript) · acp_client (fs/terminal
                     que o agente pede) · status (indicador) · bin/ agente falso
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
                     Onboarding · StatusDot · AcpPane
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
- **Sessão ACP:** UI → `acp_spawn` → o daemon sobe o adapter e guarda um
  *transcript*; a UI atacha e recebe o replay. Fechar a janela não mata a
  conversa — mesmo contrato do scrollback. Detalhes na seção "Modo ACP".
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
| Indicador | estado de cada sessão (rodando/esperando aprovação/pronto/erro) inferido do stream do PTY (`status.rs`) ou emitido pela sessão ACP; `StatusDot` na aba e no pane |
| Modo ACP | opcional: chat estruturado por JSON-RPC, com `fs/*` e `terminal/*` executados pelo Perene dentro do diretório da sessão |
| UX | menus de contexto, modais de nomeação, confirmação antes de excluir/fechar, onboarding com spotlight, i18n (en/pt-BR/es), usage de tokens |

## Modo ACP (opcional, desligado por padrão)

O modo consagrado é a CLI rodando num PTY. O **ACP** ([Agent Client
Protocol](https://agentclientprotocol.com)) é um segundo modo, ligado em
Configurações → "Modo ACP", e vale só para as ferramentas com adapter (hoje o
Claude, via `npx -y @zed-industries/claude-agent-acp`). Sessões já abertas
seguem no modo em que nasceram; o histórico sempre retoma no terminal, porque
retomar por id é um recurso da CLI.

**Por que existe:** no terminal, a CLI lê arquivos e roda comandos por conta
própria e o Perene só vê pixels. No ACP a relação inverte — o agente *pede* e
quem executa somos nós. Isso dá escopo (nada fora do diretório da sessão) e
visibilidade (cada ferramenta vira um cartão no chat).

**Onde cada coisa mora:**

| Camada | Arquivo | Responsabilidade |
|---|---|---|
| Protocolo JSON-RPC | `crates/perene-acp/src/jsonrpc.rs` | bidirecional, genérico sobre `Read`/`Write` (testável com pipes) |
| Tipos do wire | `crates/perene-acp/src/protocol.rs` | variantes desconhecidas caem em `Other` — o protocolo evolui |
| Processo do agente | `crates/perene-acp/src/agent.rs` | spawn, handshake, prompt, cancel, cauda do stderr |
| Sessão viva | `crates/perene-daemon/src/acp.rs` | transcript p/ replay, permissões, status, login shell |
| O que o agente pede | `crates/perene-daemon/src/acp_client.rs` | `fs/*` e `terminal/*`, presos ao diretório da sessão |
| IPC | `crates/perene-protocol/src/lib.rs` | `AcpSpawn/AcpPrompt/AcpCancel/AcpPermission` · `DaemonMessage::Acp` |
| UI | `apps/desktop/src/components/AcpPane.svelte` + `src/lib/acp.svelte.ts` | chat, cartões de ferramenta, diálogo de permissão |

**Decisões que não são óbvias:**

- **A sessão vive no daemon**, não na UI. Se vivesse na janela, fechá-la mataria
  a conversa — o oposto do que o Perene promete. O transcript é o análogo do
  scrollback: no reattach ele é reproduzido inteiro.
- **Permissão bloqueia o agente de verdade.** O handler roda numa thread do
  JSON-RPC e espera num canal indexado por `request_id`; a resposta da UI
  destrava aquele pedido específico. Timeout de 30 min evita prender o processo
  para sempre se ninguém responder.
- **Escopo é rede de segurança, não política.** Quem decide se a ação é
  permitida é o `session/request_permission` (vai ao usuário). O
  `acp_client.rs` só garante que nada aconteça fora do diretório da sessão —
  inclusive contra `..` e symlink, canonizando o ancestral existente.
- **Adapter pelo login shell.** Ver Gotcha #13.

**Testar:** `cargo test -p perene-daemon --test acp` roda o e2e contra um agente
falso de verdade (`src/bin/perene-fake-acp-agent.rs`) — processo, stdio e
JSON-RPC como em produção, sem rede nem conta de IA. Contra o adapter real:
`cargo test -p perene-daemon --test acp -- --ignored --nocapture`.

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

13. **O adapter ACP precisa de login shell — e de erro que diga o motivo.**
    O app aberto pelo Finder herda PATH mínimo (`/usr/bin:/bin:...`): o `npx` do
    nvm/homebrew **não existe ali**, então o ACP falharia só para quem não abre o
    Perene de um terminal. `acp.rs::login_shell_command()` envolve o adapter em
    `$SHELL -lc 'exec ...'` (com `exec` para o grupo de processos continuar
    certo). Como o shell no meio transforma "programa não encontrado" em "a
    conexão fechou", guardamos a **cauda do stderr** do adapter e a anexamos à
    falha do handshake — é ela que diz `command not found` ou
    `Invalid permissions.defaultMode`.
14. **`npx` vira `node`: matar o filho direto deixa neto vivo.** Cada pane ACP
    fechado vazava um adapter (~100 MB de node) para sempre. O agente nasce em
    process group próprio (`process_group(0)`) e o kill vai no grupo
    (`taskkill /T` no Windows). O teste
    `killing_the_pane_takes_the_whole_process_tree_down` falha se o grupo sair.
15. **O erro do JSON-RPC mora em `data`, não em `message`.** O adapter responde
    `"Internal error"` e põe o motivo real em `data.details`. `RpcError::from_wire`
    junta os dois; sem isso, toda falha vira a mesma frase inútil.
16. **`permissions.defaultMode: "auto"` quebra o adapter atual.** O
    `@zed-industries/claude-agent-acp` 0.23.1 embute um `claude-agent-sdk`
    (0.2.83) que não conhece esse valor e recusa o `session/new` com
    *"Invalid permissions.defaultMode: auto."*. É skew de versão entre o adapter
    e o Claude Code do usuário, não bug nosso — mas o modo ACP não sobe enquanto
    a chave estiver no `~/.claude/settings.json`.

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
- **Modo ACP**: só o Claude tem adapter mapeado (`profiles.ts::acpConfig`).
  Falta `session/load` (retomar conversa ACP por id, hoje o histórico cai no
  terminal), anexar imagem no prompt e renderizar markdown/diff nos cartões de
  ferramenta. E não foi possível validar um turno completo contra o adapter real
  nesta máquina por causa do Gotcha #16.
- **Fila sugerida**: (1) cwd tracking (OSC 7); (2) fallback nativo da busca
  global; (3) resolver o `.dmg` e gerar release v0.1.0 com artefatos (o job
  `bundle` roda em tags `v*` ou dispatch manual).

## Referências da v1 (não mexer — `~/Projects/tool/zampimanager`)

`Sources/Perene/Models.swift`, `docs/data-model.md`, `SessionHistory.swift`,
`UsageProvider.swift`, `GitController.swift`, `SessionHistoryController.swift`.
Os SVGs das marcas em `icons/` vieram de lá.
