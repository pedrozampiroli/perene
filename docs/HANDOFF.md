# Handoff — Perene v2 (atualizado em 2026-07-27)

> Documento pra retomar o projeto numa nova sessão. Leia junto com
> [`../PLAN.md`](../PLAN.md), [`../CLAUDE.md`](../CLAUDE.md) e o `git log`.

## Estado atual

**Todos os milestones M0–M6 do PLAN.md foram executados**, mais uma rodada
grande de melhorias pós-uso (ver `git log`).

- **Repositório:** https://github.com/pedrozampiroli/perene (**privado**).
  Tudo mergeado no `main` (29 commits); o branch de trabalho foi removido.
- **CI VERDE nas 3 plataformas** (macOS 2m9s · Ubuntu 5m9s · Windows 9m52s) —
  fecha o último critério de aceite pendente do M6.
- Working tree limpo. `cargo test --workspace` verde (11 suítes); `npm run check`
  0 erros.

```
6510894 M6: packaging + CI + RAM sob o alvo
8d96849 M5: git + visualizador de arquivos (mini-VSCode)
f032877 M4: resume pós-reboot + histórico + usage
509015d M3: UI completa (sidebar, splits, perfis, YOLO, paste de imagem)
3e569c1 M2: manifest v3 + persistência atômica no perene-core
aaa1d79 M1: daemon de sessões (PTYs sobrevivem à UI) + reattach com scrollback
68227a7 M0: esqueleto Tauri 2 + Svelte 5 + xterm.js com terminal local   (no main)
```

## Como rodar / testar

```bash
cd apps/desktop
npm install            # (já instalado)
npm run tauri dev      # roda o app em dev
npm run tauri build    # gera o instalador — sai em target/release/bundle/
npm run check          # typecheck do front (svelte-check)

# na raiz do repo:
cargo test --workspace # testes Rust
```

Instalador mac já gerado e testado: `target/release/bundle/dmg/Perene_0.1.0_aarch64.dmg`.

## Verificado ✅ vs pendente de validação manual ⏳

**Verificado (automatizado/runtime por mim):**
- Daemon sobrevive ao kill da UI; relaunch adota o mesmo daemon; reattach com
  replay de scrollback (teste automatizado `crates/perene-daemon/tests/protocol.rs`).
- Persistência: round-trip, rotação de backup, recuperação de corrupção, sem
  tocar `~/.perene2` (testes injetam tempdir).
- Resume: pane restaurado do manifest spawna `codex resume --last` /
  `claude --resume <uuid>` (+YOLO) — confirmado via `ps`.
- Histórico lê 1590 sessões reais; usage cold 2.4s / warm 16ms.
- Git: `git_status`/`git_diff`/fs testados no repo real.
- Instalador `.dmg` sobe e roda com 5 terminais a **109 MB** (< 150 MB alvo).

**Pendente (precisa de mão humana no GUI — não dá pra automatizar):**
- ⏳ **Gate do M0** (é o gate do projeto): acentos via dead keys (⌥e→é),
  Shift+Enter no Claude Code, scroll trackpad, ⌘C/⌘V, `vim`. **Task #1 segue
  aberta por causa disso.**
- ⏳ Fluxos M3/M5: splits, drag&drop, renomear, abrir/editar/⌘S/diff no viewer.
- ✅ CI verde nas 3 plataformas (feito em 2026-07-27).

## Mapa da arquitetura

```
crates/
  perene-protocol/   tipos IPC (ClientMessage/DaemonMessage, framing) + eventos
  perene-core/       models(manifest v3) · store(atômico) · settings · paths ·
                     history · usage · sqlite  — Rust puro, testável
  perene-daemon/     session(1 PTY/pane + scrollback) · server(unix socket +
                     flock single-instance) · pty(build_command login shell)
apps/desktop/
  src-tauri/src/     lib.rs(registro dos comandos) · main.rs(--daemon reexec) ·
                     client.rs(cliente do daemon) · state.rs(manifest/settings/
                     history/usage/paste) · files.rs(fs + git)
  src/lib/           store.svelte.ts(estado+ações) · terminal.ts(xterm) ·
                     editor.ts(CodeMirror) · profiles.ts · api.ts · types.ts
  src/components/    App · Sidebar · BottomBar · TabGrid · SplitContainer ·
                     PaneView · FilesPane · FileTree · SettingsModal ·
                     HistoryModal · UsageModal
```

Estado do app em `~/.perene2/` (manifest.json, settings.json, daemon.sock,
scrollback/, paste/, usage-cache.json).

## Gotchas / lições desta sessão (não repita os erros)

1. **serde `rename_all` em ENUM só renomeia VARIANTES, não os campos.**
   `LayoutNode::Leaf { pane_id }` ia como `pane_id` (snake) e o front lia
   `paneId=undefined` → terminal nunca spawnava. Fix: `#[serde(rename_all)]` em
   CADA variante. (Bug que travou o M3 por um tempo; tem teste de regressão.)
2. **RAM: o renderer WebGL do xterm é o vilão.** Com 5 terminais o WebContent
   ia a 274 MB (~360 MB total). Virou opção nas Configurações, **default OFF**
   → 109 MB. Se ligar WebGL, conta com o custo.
3. **App = daemon no MESMO binário** (`perene-desktop --daemon`), sem sidecar.
   `client.rs::daemon_command()` reexecuta o próprio exe. `PERENE_DAEMON_BIN`
   permite apontar um binário standalone (usado por testes/dev).
4. **Medir RAM: rodar o `.app` via `open`**, não o binário cru em background
   (binário backgrounded morre por SIGHUP quando o shell sai). Use
   `vmmap --summary <pid>` (phys_footprint), não RSS (superconta páginas
   compartilhadas). Atribua os helpers WebKit por PID > pid-do-app (ou kill-diff).
5. **Existem DOIS apps chamados "Perene"**: a v1 (Swift, `~/Projects/tool/
   zampimanager`, em uso pelo usuário) e esta v2. Cuidado ao dar `pkill`/
   screenshot — filtre por `target/.../perene-desktop`.
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
- **Ícone**: já é o ícone real do Perene (importado da v1).
- **Busca global no Windows sem ripgrep**: `search_in_files` usa `rg` e, sem ele,
  cai no `grep -rn` — que não existe no Windows. Sem `rg` instalado, o ⌘⇧F
  devolve vazio (não quebra, só não acha nada). Falta um fallback nativo.
- **Fila de próximos passos sugerida**: (1) cwd tracking (OSC 7); (2) fallback
  nativo da busca global; (3) gerar release com artefatos (o job `bundle` roda
  em tags `v*` ou dispatch manual).

## Referências da v1 (não mexer — `~/Projects/tool/zampimanager`)

`Sources/Perene/Models.swift`, `docs/data-model.md`, `SessionHistory.swift`,
`UsageProvider.swift`, `GitController.swift`, `SessionHistoryController.swift`.
