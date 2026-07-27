//! Configurações globais do app (não pertencem a um manifest específico).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Preferências do usuário, em `~/.perene2/settings.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Modo YOLO: adiciona a flag de "pular permissões" ao spawnar CLIs de IA.
    pub yolo: bool,
    /// Tamanho da fonte dos terminais.
    pub font_size: u16,
    /// Renderizador WebGL do xterm: mais rápido, porém MUITO mais RAM (com 5
    /// terminais o WebContent passa de 270 MB). Desligado por padrão para caber
    /// no alvo de RAM; o usuário pode ligar se preferir performance.
    pub webgl: bool,
    /// Programa do shell a usar (ex.: `/bin/bash`, `wsl.exe`). Vazio = padrão do
    /// sistema (`$SHELL` / PowerShell).
    #[serde(default)]
    pub shell: String,
    /// Perguntar sobre criar worktree isolada ao abrir nova sessão.
    #[serde(default = "default_true")]
    pub ask_worktree: bool,
    /// Largura da sidebar em px (arrastável).
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,
    /// Largura do painel lateral do editor (árvore/mudanças) em px.
    #[serde(default = "default_editor_panel_width")]
    pub editor_panel_width: u32,
}

fn default_true() -> bool {
    true
}
fn default_sidebar_width() -> u32 {
    240
}
fn default_editor_panel_width() -> u32 {
    240
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            yolo: false,
            font_size: 13,
            webgl: false,
            shell: String::new(),
            ask_worktree: true,
            sidebar_width: default_sidebar_width(),
            editor_panel_width: default_editor_panel_width(),
        }
    }
}

/// Store de settings (mesma escrita atômica + backup do manifest).
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn at_state_dir() -> Self {
        Self::new(crate::paths::state_dir().join("settings.json"))
    }

    /// Carrega as settings; devolve o default se o arquivo não existe.
    pub fn load(&self) -> std::io::Result<Settings> {
        Ok(crate::store::read_json_with_backup(&self.path)?.unwrap_or_default())
    }

    pub fn save(&self, settings: &Settings) -> std::io::Result<()> {
        crate::store::write_json_atomic(&self.path, settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        assert_eq!(store.load().unwrap(), Settings::default());
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        let s = Settings {
            yolo: true,
            font_size: 15,
            webgl: true,
            shell: "/bin/bash".into(),
            ask_worktree: false,
            sidebar_width: 300,
            editor_panel_width: 280,
        };
        store.save(&s).unwrap();
        assert_eq!(store.load().unwrap(), s);
    }
}
