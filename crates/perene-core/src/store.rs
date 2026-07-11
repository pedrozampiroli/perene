//! Persistência atômica: escrita `tmp + fsync + rename` + 1 backup `.bak`, com
//! fallback ao backup se o principal corromper (lição #3).
//!
//! O caminho é **injetado** (nunca hardcode de `~/.perene2`): testes usam
//! diretórios temporários e jamais tocam o estado real (lição #1).

use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::models::Manifest;

fn backup_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".bak");
    PathBuf::from(s)
}

fn tmp_of(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".tmp");
    PathBuf::from(s)
}

/// Grava `value` como JSON em `path`, atomicamente, rotacionando `.bak`.
///
/// 1. Escreve `<path>.tmp` (+ fsync). 2. Copia o principal atual → `.bak`.
/// 3. `rename(tmp, path)` — troca atômica. Crash entre 2 e 3 deixa o principal
/// intacto e o `.bak` com o estado anterior.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let tmp = tmp_of(path);
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    if path.exists() {
        let _ = std::fs::copy(path, backup_of(path)); // best-effort
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Lê JSON de `path`. `None` se não existe. Se o principal corromper, tenta `.bak`.
pub fn read_json_with_backup<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice::<T>(&bytes) {
            Ok(v) => Ok(Some(v)),
            Err(_) => read_backup(path),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn read_backup<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match std::fs::read(backup_of(path)) {
        Ok(bytes) => serde_json::from_slice::<T>(&bytes)
            .map(Some)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "arquivo principal corrompido e sem backup",
        )),
        Err(e) => Err(e),
    }
}

/// Guarda o manifest em disco de forma atômica e resiliente.
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

    pub fn backup_path(&self) -> PathBuf {
        backup_of(&self.path)
    }

    /// Carrega o manifest. `None` se ainda não existe; fallback ao `.bak` se
    /// o principal corromper.
    pub fn load(&self) -> io::Result<Option<Manifest>> {
        read_json_with_backup(&self.path)
    }

    /// Salva o manifest atomicamente, rotacionando o backup do estado anterior.
    pub fn save(&self, manifest: &Manifest) -> io::Result<()> {
        write_json_atomic(&self.path, manifest)
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

        assert_eq!(store.load().unwrap().unwrap().workspaces[0].name, "segundo");
        let bak_bytes = std::fs::read(store.backup_path()).unwrap();
        let bak: Manifest = serde_json::from_slice(&bak_bytes).unwrap();
        assert_eq!(bak.workspaces[0].name, "primeiro");
    }

    #[test]
    fn corrupt_main_falls_back_to_backup() {
        let (_dir, store) = temp_store();
        let m1 = Manifest::bootstrap("/tmp/good");
        store.save(&m1).unwrap();
        let mut m2 = Manifest::bootstrap("/tmp/good2");
        m2.workspaces[0].name = "novo".into();
        store.save(&m2).unwrap();

        std::fs::write(store.path(), b"{ isto nao e json valido").unwrap();

        let recovered = store.load().unwrap().unwrap();
        assert_eq!(recovered.workspaces[0].name, "Perene"); // m1 default
    }

    #[test]
    fn does_not_touch_real_state_dir() {
        let (dir, store) = temp_store();
        assert!(store.path().starts_with(dir.path()));
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();
        assert!(!store.path().starts_with(home.join(".perene2")));
    }
}
