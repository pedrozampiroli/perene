<div align="center">

# 🌲 Perene

**A cross-platform terminal manager for AI coding CLIs.**

Run Claude Code, Codex, OpenCode and plain shells side by side — organized in
workspaces, folders, tabs and split panes, with sessions that **survive closing
the window**.

[![CI](https://github.com/pedrozampiroli/perene/actions/workflows/ci.yml/badge.svg)](https://github.com/pedrozampiroli/perene/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-blue)
![RAM](https://img.shields.io/badge/RAM-~109%20MB%20with%205%20terminals-brightgreen)

*Built with Tauri 2 · Rust · Svelte 5 · xterm.js · CodeMirror 6*

</div>

---

## Why

Juggling several AI coding agents means juggling several terminals — and losing
them the moment something crashes or you close the window. Perene keeps every
session alive in a **background daemon**, so closing the UI never kills your
work, and reopening reattaches with full scrollback.

It is a ground-up rewrite of a macOS-only Swift app, rebuilt to run **everywhere**
without the memory cost of Electron: it uses the system webview (WKWebView,
WebView2, WebKitGTK) and targets **under 150 MB with 5 terminals** — currently
landing around **109 MB**.

## Features

**Terminals & sessions**
- 🔌 **Sessions survive the UI.** A Rust daemon owns one PTY per pane. Close the
  window, reopen — same processes, same scrollback.
- 🔁 **Resume after reboot.** Panes come back and the CLIs resume the *same
  conversation* (`claude --resume`, `codex resume --last`, `opencode --continue`).
- 🧩 **Workspaces → folders → tabs → split panes**, with drag & drop, layout
  presets and draggable dividers.
- 🎛️ **Tool profiles** for Claude / Codex / OpenCode / shell, each with its own
  icon and brand color, plus an optional **YOLO** switch for skipping approvals.
- ⌨️ Proper terminal behavior: dead keys and IME (`⌥e` → `é`), true color, 256
  colors, `vim`, mouse selection, and **Shift+Enter** newlines in Claude Code
  (via the CSI-u sequence).
- 🖼️ **Paste an image** with `⌘V` — it's saved to disk and the path is typed in.

**Git & editor**
- 🌿 **Git everywhere**: branch, ahead/behind and dirty state in the title bar of
  *any* pane, with a menu for switch/create branch, fetch, pull, push and PRs.
- 🪴 **Isolated worktrees per session.** Starting a session offers to create a
  worktree in `.perene/worktrees/` (auto-added to `.gitignore`) so an agent can
  work without touching your tree — and you can open the editor *inside* it to
  watch what it's doing.
- 📝 **Built-in editor** with file tabs, syntax highlighting, `⌘S`, side-by-side
  diffs, commit log and a commit box.
- 🔎 **Editor shortcuts you already know**: `⌘P` fuzzy file open, `⌘⇧F` project
  search (ripgrep), `⌘⇧H` project replace, `⌘F` / `⌘H` in-file.

**Everything else**
- 📊 **Token usage** across all three harnesses, with a disk cache (~2.4s cold,
  ~16ms warm over 1500+ sessions).
- 🕐 **Session history** with search, preview and one-click resume.
- 🌍 **i18n** — English, Português (BR) and Español. Adding a language is
  literally copying one JSON file (see below).

## Install

Download the installer from [Releases](../../releases), or build it yourself:

```bash
git clone https://github.com/pedrozampiroli/perene.git
cd perene/apps/desktop
npm install
npm run tauri build     # → .dmg (macOS) · .msi/NSIS (Windows) · .deb/AppImage (Linux)
```

**Requirements:** [Rust](https://rustup.rs), Node 22+, and on Linux the WebKitGTK
dev packages (`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`,
`patchelf`).

## Development

```bash
cd apps/desktop
npm install
npm run tauri dev       # run the app with hot reload
npm run check           # typecheck the frontend

cargo test --workspace  # Rust tests
cargo build --workspace
```

App state lives in `~/.perene2/` (`%APPDATA%\perene2\` on Windows): manifest,
settings, scrollback and pasted images — all written atomically with a `.bak`.

## Architecture

```
┌──────────────────────────────┐        ┌───────────────────────────────┐
│  apps/desktop (Tauri 2)      │        │  perene-daemon                │
│  ┌────────────────────────┐  │  IPC   │  ┌─────────────────────────┐  │
│  │ Svelte 5 + xterm.js    │◄─┼────────┼─►│ 1 PTY per pane          │  │
│  │ CodeMirror 6 editor    │  │  JSON  │  │ scrollback buffer       │  │
│  └────────────────────────┘  │ lines  │  │ attach / detach / replay│  │
│  src-tauri = thin client     │        │  └─────────────────────────┘  │
└──────────────────────────────┘        │  survives the UI · single     │
                                        │  instance (flock)             │
        ┌───────────────────┐           └───────────────────────────────┘
        │  perene-core      │   models (manifest v3) · atomic storage
        │  (pure Rust)      │   history · token usage · paths
        └───────────────────┘
```

The UI is deliberately **disposable**: all logic lives in pure Rust crates, so a
native frontend could replace it without a rewrite. The app and the daemon are
the *same binary* — the UI re-executes itself with `--daemon`, so there is no
sidecar to ship.

| Crate / app | Role |
|---|---|
| `crates/perene-core` | Data model, atomic persistence, history, usage, paths |
| `crates/perene-protocol` | Shared UI ⇄ daemon message types (JSON-lines) |
| `crates/perene-daemon` | Session server: PTYs, scrollback, attach/detach |
| `apps/desktop` | Tauri shell + Svelte UI |

## Shortcuts

A few highlights — the full list is in the app (`⌘,`) and in
[`docs/atalhos.md`](docs/atalhos.md).

| macOS | Windows/Linux | Action |
|---|---|---|
| `⌘T` | `Ctrl+Shift+T` | New session |
| `⌘D` / `⌘⇧D` | `Ctrl+Shift+D` / `Ctrl+Alt+D` | Split right / down |
| `⌘P` | `Ctrl+P` | Go to file |
| `⌘⇧F` / `⌘⇧H` | `Ctrl+Shift+F` / `Ctrl+Shift+H` | Search / replace in project |
| `⌘Y` / `⌘U` | `Ctrl+Shift+Y` / `Ctrl+Shift+U` | Session history / token usage |
| `⇧Enter` | `⇧Enter` | New line without submitting |

## Translating

Copy `apps/desktop/src/i18n/en.json` to `<locale>.json`, translate the values,
and it shows up in Settings automatically — no code changes. Missing keys fall
back to English.

```jsonc
{
  "$meta": { "name": "Français", "flag": "🇫🇷" },
  "sidebar.workspaces": "Espaces de travail",
  // …
}
```

## Status

macOS is the fully exercised platform. Windows and Linux **build and pass tests
in CI**, but the daemon's IPC transport is still Unix-socket only — Windows named
pipes are the main piece missing for full runtime support there.

## License

[MIT](LICENSE) © Pedro Zampiroli
