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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            yolo: false,
            font_size: 13,
            webgl: false,
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
        };
        store.save(&s).unwrap();
        assert_eq!(store.load().unwrap(), s);
    }
}
