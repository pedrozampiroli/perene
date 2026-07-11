//! Persistência do manifest: escrita atômica (tmp + rename) + 1 backup `.bak`,
//! com fallback ao backup se o principal corromper (lição #3).
//!
//! O caminho é **injetado** (nunca hardcode de `~/.perene2`): testes usam
//! diretórios temporários e jamais tocam o estado real (lição #1).

use std::io;
use std::path::{Path, PathBuf};

use crate::models::Manifest;

/// Guarda um manifest em disco de forma atômica e resiliente.
pub struct ManifestStore {
    path: PathBuf,
}

impl ManifestStore {
    /// Store apontando para um arquivo específico (injeção — usado por testes).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Store de produção: `<state_dir>/manifest.json`.
    pub fn at_state_dir() -> Self {
        Self::new(crate::paths::state_dir().join("manifest.json"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn backup_path(&self) -> PathBuf {
        self.path.with_extension("json.bak")
    }

    fn tmp_path(&self) -> PathBuf {
        self.path.with_extension("json.tmp")
    }

    /// Carrega o manifest. `None` se ainda não existe. Se o principal estiver
    /// corrompido, tenta o `.bak` antes de falhar.
    pub fn load(&self) -> io::Result<Option<Manifest>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => match serde_json::from_slice::<Manifest>(&bytes) {
                Ok(m) => Ok(Some(m)),
                Err(_) => self.load_backup(),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn load_backup(&self) -> io::Result<Option<Manifest>> {
        match std::fs::read(self.backup_path()) {
            Ok(bytes) => serde_json::from_slice::<Manifest>(&bytes)
                .map(Some)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "manifest principal corrompido e sem backup",
            )),
            Err(e) => Err(e),
        }
    }

    /// Salva o manifest atomicamente, rotacionando o backup do estado anterior.
    ///
    /// 1. Escreve em `manifest.json.tmp` (+ fsync).
    /// 2. Copia o principal atual → `.bak` (backup do que já estava lá).
    /// 3. `rename(tmp, principal)` — troca atômica.
    ///
    /// Um crash entre 2 e 3 deixa `.bak` = estado anterior e `principal` intacto.
    pub fn save(&self, manifest: &Manifest) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bytes = serde_json::to_vec_pretty(manifest)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let tmp = self.tmp_path();
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }

        // Backup do estado anterior (best-effort — não aborta o save).
        if self.path.exists() {
            let _ = std::fs::copy(&self.path, self.backup_path());
        }

        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Manifest;

    fn temp_store() -> (tempfile::TempDir, ManifestStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = ManifestStore::new(dir.path().join("manifest.json"));
        (dir, store)
    }

    #[test]
    fn load_missing_is_none() {
        let (_dir, store) = temp_store();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let (_dir, store) = temp_store();
        let m = Manifest::bootstrap("/tmp/x");
        store.save(&m).unwrap();
        let back = store.load().unwrap().unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn second_save_rotates_backup_with_previous_state() {
        let (_dir, store) = temp_store();
        let mut m1 = Manifest::bootstrap("/tmp/one");
        m1.workspaces[0].name = "primeiro".into();
        store.save(&m1).unwrap();

        let mut m2 = Manifest::bootstrap("/tmp/two");
        m2.workspaces[0].name = "segundo".into();
        store.save(&m2).unwrap();

        // principal = m2
        assert_eq!(store.load().unwrap().unwrap().workspaces[0].name, "segundo");
        // .bak = m1 (estado anterior)
        let bak_bytes = std::fs::read(store.backup_path()).unwrap();
        let bak: Manifest = serde_json::from_slice(&bak_bytes).unwrap();
        assert_eq!(bak.workspaces[0].name, "primeiro");
    }

    #[test]
    fn corrupt_main_falls_back_to_backup() {
        let (_dir, store) = temp_store();
        let m1 = Manifest::bootstrap("/tmp/good");
        store.save(&m1).unwrap();
        // Gera um .bak (segundo save).
        let mut m2 = Manifest::bootstrap("/tmp/good2");
        m2.workspaces[0].name = "novo".into();
        store.save(&m2).unwrap();

        // Corrompe o principal.
        std::fs::write(store.path(), b"{ isto nao e json valido").unwrap();

        // load() deve cair no .bak (que tem m1).
        let recovered = store.load().unwrap().unwrap();
        assert_eq!(recovered.workspaces[0].name, "Perene"); // m1 default name
    }

    #[test]
    fn does_not_touch_real_state_dir() {
        // Sanidade da lição #1: o store de teste aponta pra um temp, não pro home.
        let (dir, store) = temp_store();
        assert!(store.path().starts_with(dir.path()));
        assert!(!store.path().starts_with(
            dirs_home().unwrap_or_default().join(".perene2")
        ));
    }

    fn dirs_home() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
