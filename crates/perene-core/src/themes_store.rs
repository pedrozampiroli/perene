//! Catálogo de temas: os embutidos mais os que o usuário importou.
//!
//! Temas do usuário vivem em `<state>/themes/*.json`, um arquivo por tema, com a
//! mesma escrita atômica do resto do app. O diretório é injetado (nunca deduzido
//! aqui dentro) pra que os testes jamais toquem no estado real — regra 1 do
//! projeto.

use std::path::{Path, PathBuf};

use crate::store::write_json_atomic;
use crate::theme::Theme;

pub struct ThemesStore {
    dir: PathBuf,
}

impl ThemesStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn at_state_dir() -> Self {
        Self::new(crate::paths::state_dir().join("themes"))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Todos os temas disponíveis: embutidos primeiro, depois os importados em
    /// ordem alfabética. Um arquivo corrompido é ignorado — um tema quebrado não
    /// pode impedir o app de listar os outros.
    pub fn list(&self) -> Vec<Theme> {
        let mut out = Theme::builtins();
        let mut user = self.list_user();
        user.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        // Um tema do usuário com id de embutido substitui o embutido, em vez de
        // aparecer duplicado no seletor.
        for t in user {
            match out.iter().position(|b| b.id == t.id) {
                Some(i) => out[i] = t,
                None => out.push(t),
            }
        }
        out
    }

    fn list_user(&self) -> Vec<Theme> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|raw| serde_json::from_str::<Theme>(&raw).ok())
            .collect()
    }

    /// Busca por id, embutido ou do usuário.
    pub fn get(&self, id: &str) -> Option<Theme> {
        self.list().into_iter().find(|t| t.id == id)
    }

    /// Grava um tema do usuário. Devolve o caminho gravado.
    pub fn save(&self, theme: &Theme) -> std::io::Result<PathBuf> {
        let path = self.dir.join(format!("{}.json", theme.id));
        write_json_atomic(&path, theme)?;
        Ok(path)
    }

    /// Importa um arquivo de tema do Zed (uma família = vários temas) e grava
    /// todos. Devolve os temas importados.
    pub fn import_zed_file(&self, path: &Path) -> Result<Vec<Theme>, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let themes = Theme::from_zed_json(&raw)?;
        for t in &themes {
            self.save(t).map_err(|e| e.to_string())?;
        }
        Ok(themes)
    }

    /// Remove um tema do usuário. Embutidos não podem ser removidos.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        if Theme::builtin(id).is_some() && !self.dir.join(format!("{id}.json")).exists() {
            return Err("tema embutido não pode ser removido".into());
        }
        let path = self.dir.join(format!("{id}.json"));
        std::fs::remove_file(&path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const ZED_FAMILY: &str = r##"{
      "name": "Ayu",
      "themes": [{
        "name": "Ayu Mirage", "appearance": "dark",
        "style": {"background": "#1f2430ff", "syntax": {"keyword": {"color": "#ffa759ff"}}}
      }]
    }"##;

    #[test]
    fn list_without_dir_returns_only_builtins() {
        let tmp = TempDir::new().unwrap();
        let store = ThemesStore::new(tmp.path().join("nao-existe"));
        assert_eq!(store.list().len(), Theme::builtins().len());
    }

    #[test]
    fn import_zed_then_list_and_get() {
        let tmp = TempDir::new().unwrap();
        let store = ThemesStore::new(tmp.path().join("themes"));

        let src = tmp.path().join("ayu.json");
        std::fs::write(&src, ZED_FAMILY).unwrap();

        let imported = store.import_zed_file(&src).unwrap();
        assert_eq!(imported.len(), 1);
        let id = imported[0].id.clone();

        let listed = store.list();
        assert_eq!(listed.len(), Theme::builtins().len() + 1);
        let got = store.get(&id).expect("tema importado deve ser encontrado");
        assert_eq!(got.ui.bg, "#1f2430");
        assert_eq!(got.syntax.keyword, "#ffa759");
    }

    #[test]
    fn user_theme_overrides_builtin_with_same_id() {
        let tmp = TempDir::new().unwrap();
        let store = ThemesStore::new(tmp.path().join("themes"));

        let mut custom = Theme::dark_plus();
        custom.ui.bg = "#000000".into();
        store.save(&custom).unwrap();

        let listed = store.list();
        assert_eq!(listed.len(), Theme::builtins().len(), "não pode duplicar");
        assert_eq!(store.get("dark-plus").unwrap().ui.bg, "#000000");
    }

    #[test]
    fn corrupt_theme_file_is_skipped_not_fatal() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("themes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("quebrado.json"), "{ isso não é json").unwrap();

        let store = ThemesStore::new(&dir);
        assert_eq!(store.list().len(), Theme::builtins().len());
    }

    #[test]
    fn builtin_cannot_be_removed() {
        let tmp = TempDir::new().unwrap();
        let store = ThemesStore::new(tmp.path().join("themes"));
        assert!(store.remove("dark-plus").is_err());
    }

    #[test]
    fn save_writes_inside_the_injected_dir_only() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("themes");
        let store = ThemesStore::new(&dir);
        let path = store.save(&Theme::one_dark()).unwrap();
        assert!(path.starts_with(&dir));
    }
}
