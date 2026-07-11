//! `perene-core` — modelos de dados e persistência do Perene v2.
//!
//! Rust puro, sem dependência de UI. O manifest v3 (Workspace → Folders → Tabs →
//! Panes) e a persistência atômica são portados no M2. No M0 este crate existe só
//! para fixar a estrutura do workspace e gerar IDs imutáveis.

use uuid::Uuid;

/// Versão do formato de manifest gravado em disco. Bump só com migração.
pub const MANIFEST_VERSION: u32 = 3;

/// Gera um ID imutável com prefixo por tipo (`ws_`, `fold_`, `tab_`, `pane_`,
/// `split_`). IDs nunca são reciclados — lição da v1.
pub fn new_id(prefix: &str) -> String {
    let short = Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &short[..12])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_prefixed_and_unique() {
        let a = new_id("pane");
        let b = new_id("pane");
        assert!(a.starts_with("pane_"));
        assert_ne!(a, b);
    }
}
