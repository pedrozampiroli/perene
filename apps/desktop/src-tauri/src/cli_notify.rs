//! Liga o "terminal bell" (BEL, `\x07`) do claude/codex/opencode: é o sinal que
//! o xterm.js do pane escuta (`onBell`) para disparar a notificação de idle
//! (ver `terminal.ts`/`PaneView.svelte`). Sem isso as 3 CLIs não tocam nada
//! dentro do pty embutido do Perene (não é um terminal com OSC9 reconhecido
//! tipo Ghostty/Kitty/iTerm2).
//!
//! Só ADICIONA a chave que falta em cada config — nunca sobrescreve uma
//! preferência que o usuário já tenha setado, e nunca mexe se o arquivo
//! existente estiver corrompido (melhor deixar sem bell do que estragar a
//! config de uma ferramenta que o usuário usa fora do Perene também).

use std::io;
use std::path::{Path, PathBuf};

use perene_core::paths::home_dir;
use perene_core::store::{read_json_with_backup, write_json_atomic};
use serde_json::{json, Value};

fn claude_settings_path() -> PathBuf {
    home_dir().join(".claude").join("settings.json")
}

fn codex_config_path() -> PathBuf {
    std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".codex"))
        .join("config.toml")
}

fn opencode_config_path() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(base).join("opencode").join("opencode.json")
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".config"));
        base.join("opencode").join("opencode.json")
    }
}

/// `claude config set --global preferredNotifChannel terminal_bell` via edição
/// direta do settings.json (idempotente, atômica).
fn ensure_claude_bell(path: &Path) -> io::Result<bool> {
    let mut val: Value = match read_json_with_backup(path) {
        Ok(Some(v)) => v,
        Ok(None) => json!({}),
        Err(e) => return Err(e), // json corrompido (principal + .bak): não mexe
    };
    let obj = match val.as_object_mut() {
        Some(o) => o,
        None => return Ok(false), // settings.json não é um objeto: não mexe
    };
    if obj.contains_key("preferredNotifChannel") {
        return Ok(false);
    }
    obj.insert("preferredNotifChannel".into(), json!("terminal_bell"));
    write_json_atomic(path, &val)?;
    Ok(true)
}

/// `[tui] notifications = true` + `notification_method = "bel"` no config.toml
/// do Codex (Rust CLI). Sem `notifications = true` o Codex não notifica nada;
/// `"bel"` força BEL em vez do OSC9 (que o pty embutido não reconhece).
fn ensure_codex_bell(path: &Path) -> io::Result<bool> {
    let existing = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let mut doc: toml::Value = if existing.trim().is_empty() {
        toml::Value::Table(Default::default())
    } else {
        match existing.parse() {
            Ok(v) => v,
            Err(_) => return Ok(false), // toml corrompido: não mexe
        }
    };
    let table = match doc.as_table_mut() {
        Some(t) => t,
        None => return Ok(false),
    };
    let tui = table
        .entry("tui")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let tui_table = match tui.as_table_mut() {
        Some(t) => t,
        None => return Ok(false), // "tui" existe mas não é uma tabela: não mexe
    };

    let mut changed = false;
    if !tui_table.contains_key("notifications") {
        tui_table.insert("notifications".into(), toml::Value::Boolean(true));
        changed = true;
    }
    if !tui_table.contains_key("notification_method") {
        tui_table.insert(
            "notification_method".into(),
            toml::Value::String("bel".into()),
        );
        changed = true;
    }
    if !changed {
        return Ok(false);
    }

    let text =
        toml::to_string_pretty(&doc).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_text_atomic(path, &text)?;
    Ok(true)
}

/// Plugin comunitário `opencode-bell` (o OpenCode não tem bell nativo) —
/// versão fixada (`@0.1.0`) de propósito: puxar sempre a última versão de um
/// plugin de terceiros arriscaria trocar de comportamento sem o usuário saber.
const OPENCODE_BELL_PLUGIN: &str = "opencode-bell@0.1.0";

fn ensure_opencode_bell(path: &Path) -> io::Result<bool> {
    let mut val: Value = match read_json_with_backup(path) {
        Ok(Some(v)) => v,
        Ok(None) => json!({ "$schema": "https://opencode.ai/config.json" }),
        Err(e) => return Err(e),
    };
    let obj = match val.as_object_mut() {
        Some(o) => o,
        None => return Ok(false),
    };
    let plugins = obj.entry("plugin").or_insert_with(|| json!([]));
    let arr = match plugins.as_array_mut() {
        Some(a) => a,
        None => return Ok(false), // "plugin" existe mas não é array: não mexe
    };
    let already = arr
        .iter()
        .any(|v| v.as_str().is_some_and(|s| s.starts_with("opencode-bell")));
    if already {
        return Ok(false);
    }
    arr.push(json!(OPENCODE_BELL_PLUGIN));
    write_json_atomic(path, &val)?;
    Ok(true)
}

fn tmp_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".tmp");
    PathBuf::from(s)
}

/// Mesma estratégia do `write_json_atomic` do perene-core (tmp + rename), só
/// que para texto cru — o config.toml do Codex não é JSON.
fn write_text_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_of(path);
    std::fs::write(&tmp, text.as_bytes())?;
    if path.exists() {
        let mut bak = path.as_os_str().to_os_string();
        bak.push(".bak");
        let _ = std::fs::copy(path, PathBuf::from(bak));
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Roda uma vez na subida do app (thread própria — I/O não pode atrasar a
/// janela). Cada CLI é independente: uma falhando não impede as outras.
pub fn ensure_all() {
    if let Err(e) = ensure_claude_bell(&claude_settings_path()) {
        eprintln!("cli_notify: não deu pra configurar o bell do claude: {e}");
    }
    if let Err(e) = ensure_codex_bell(&codex_config_path()) {
        eprintln!("cli_notify: não deu pra configurar o bell do codex: {e}");
    }
    if let Err(e) = ensure_opencode_bell(&opencode_config_path()) {
        eprintln!("cli_notify: não deu pra configurar o bell do opencode: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_creates_settings_with_bell_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert!(ensure_claude_bell(&path).unwrap());
        let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["preferredNotifChannel"], "terminal_bell");
    }

    #[test]
    fn claude_preserves_existing_keys_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"model": "opus", "yolo": true}"#).unwrap();

        assert!(ensure_claude_bell(&path).unwrap());
        let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["model"], "opus");
        assert_eq!(v["yolo"], true);
        assert_eq!(v["preferredNotifChannel"], "terminal_bell");

        // Segunda chamada: nada muda (idempotente).
        assert!(!ensure_claude_bell(&path).unwrap());
    }

    #[test]
    fn claude_never_overrides_users_explicit_channel() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"preferredNotifChannel": "desktop"}"#).unwrap();
        assert!(!ensure_claude_bell(&path).unwrap());
        let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["preferredNotifChannel"], "desktop");
    }

    #[test]
    fn codex_creates_config_with_bell_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert!(ensure_codex_bell(&path).unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        let doc: toml::Value = text.parse().unwrap();
        assert_eq!(doc["tui"]["notifications"].as_bool(), Some(true));
        assert_eq!(doc["tui"]["notification_method"].as_str(), Some("bel"));
    }

    #[test]
    fn codex_preserves_existing_toml_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "model = \"o3\"\n\n[tui]\nfoo = 1\n").unwrap();

        assert!(ensure_codex_bell(&path).unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        let doc: toml::Value = text.parse().unwrap();
        assert_eq!(doc["model"].as_str(), Some("o3"));
        assert_eq!(doc["tui"]["foo"].as_integer(), Some(1));
        assert_eq!(doc["tui"]["notifications"].as_bool(), Some(true));

        assert!(!ensure_codex_bell(&path).unwrap());
    }

    #[test]
    fn codex_respects_users_explicit_notification_method() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[tui]\nnotification_method = \"osc9\"\n").unwrap();
        assert!(ensure_codex_bell(&path).unwrap()); // "notifications" ainda falta
        let text = std::fs::read_to_string(&path).unwrap();
        let doc: toml::Value = text.parse().unwrap();
        assert_eq!(doc["tui"]["notification_method"].as_str(), Some("osc9"));
        assert_eq!(doc["tui"]["notifications"].as_bool(), Some(true));
    }

    #[test]
    fn opencode_adds_plugin_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.json");
        assert!(ensure_opencode_bell(&path).unwrap());
        let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["plugin"][0], OPENCODE_BELL_PLUGIN);
    }

    #[test]
    fn opencode_preserves_other_plugins_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.json");
        std::fs::write(&path, r#"{"plugin": ["other-plugin@1.0.0"]}"#).unwrap();

        assert!(ensure_opencode_bell(&path).unwrap());
        let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let arr = v["plugin"].as_array().unwrap();
        assert!(arr.iter().any(|p| p == "other-plugin@1.0.0"));
        assert!(arr.iter().any(|p| p == OPENCODE_BELL_PLUGIN));

        assert!(!ensure_opencode_bell(&path).unwrap());
    }

    #[test]
    fn corrupted_files_are_left_untouched() {
        let dir = tempfile::tempdir().unwrap();

        let claude = dir.path().join("settings.json");
        std::fs::write(&claude, "{ not json").unwrap();
        assert!(ensure_claude_bell(&claude).is_err());
        assert_eq!(std::fs::read_to_string(&claude).unwrap(), "{ not json");

        let codex = dir.path().join("config.toml");
        std::fs::write(&codex, "not = [valid toml").unwrap();
        assert!(!ensure_codex_bell(&codex).unwrap());
        assert_eq!(
            std::fs::read_to_string(&codex).unwrap(),
            "not = [valid toml"
        );
    }
}
