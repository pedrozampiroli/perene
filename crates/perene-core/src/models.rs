//! Manifest v3 — a fonte da verdade do layout/sessões.
//!
//! Portado do v1 (Swift `Models.swift`): Workspace → Folders → Tabs → Panes, com
//! `LayoutNode` (árvore binária de splits por aba). Diferenças do v1:
//!  - **Sem `tmuxSession`**: o daemon indexa panes pelo próprio `paneId` imutável.
//!  - Nomes limpos (`harnessSessionId` no lugar de `claudeSessionName`) — v3 é
//!    manifest novo; migração automática do v1 está fora de escopo (PLAN §7).
//!
//! Serialização em `camelCase` para casar com o front (Svelte/TS) sem conversão.

use serde::{Deserialize, Serialize};

/// Alias de ID imutável (`ws_`/`fold_`/`tab_`/`pane_`/`split_`). Ver [`crate::new_id`].
pub type Id = String;

/// Raiz persistida em `~/.perene2/manifest.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub active_workspace_id: Option<Id>,
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: crate::MANIFEST_VERSION,
            active_workspace_id: None,
            workspaces: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: Id,
    pub name: String,
    pub order: i32,
    #[serde(default)]
    pub folders: Vec<Folder>,
    #[serde(default)]
    pub tabs: Vec<Tab>,
    /// Aba focada, restaurada no boot.
    #[serde(default)]
    pub active_tab_id: Option<Id>,
    /// Diretório onde novos terminais abrem (o caminho do projeto).
    #[serde(default)]
    pub directory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: Id,
    pub name: String,
    pub order: i32,
    #[serde(default)]
    pub collapsed: bool,
    /// Sobrepõe o diretório do workspace para terminais desta pasta.
    #[serde(default)]
    pub directory: Option<String>,
}

/// Item da sidebar: grupo nomeado de panes num grid. Só a aba ativa é exibida.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: Id,
    #[serde(default)]
    pub folder_id: Option<Id>,
    pub title: String,
    #[serde(default)]
    pub panes: Vec<Pane>,
    /// Grid de painéis desta aba; as folhas referenciam `pane.id`.
    pub layout: LayoutNode,
    #[serde(default)]
    pub active_pane_id: Option<Id>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

/// Tipo de pane: terminal (default) ou visualizador de arquivos (M5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PaneKind {
    #[default]
    Terminal,
    Files,
}

/// Um terminal = uma sessão do daemon (indexada por `id`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pane {
    pub id: Id,
    #[serde(default)]
    pub kind: PaneKind,
    /// Perfil de ferramenta: `shell` / `claude` / `codex` / `opencode`.
    pub tool_profile_id: String,
    /// Fonte da verdade do cwd para re-spawn; mantido atual por cwd tracking.
    pub working_directory: String,
    /// Id de sessão do harness fixado — Claude `--session-id`/`--resume`, ou a
    /// sessão do histórico sendo retomada.
    #[serde(default)]
    pub harness_session_id: Option<String>,
    /// `true` quando o pane retoma uma sessão existente (aberta do histórico) em
    /// vez de criar uma nova.
    #[serde(default)]
    pub resume_existing: bool,
    /// Caminho do dump de scrollback (resume pós-reboot, M4).
    #[serde(default)]
    pub scrollback_file: Option<String>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
}

/// Direção de um split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SplitDirection {
    /// Painéis lado a lado (divisor vertical).
    Horizontal,
    /// Painéis empilhados (divisor horizontal).
    Vertical,
}

/// Árvore binária de splits de uma aba. Folhas referenciam panes; nós de split
/// carregam um id estável para a UI persistir o `ratio` ao arrastar o divisor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LayoutNode {
    // rename_all no ENUM só renomeia as VARIANTES; para os CAMPOS internos
    // (pane_id → paneId) o rename_all precisa vir em cada variante — senão o
    // front (TS) lê `paneId` como undefined.
    #[serde(rename_all = "camelCase")]
    Leaf {
        pane_id: Id,
    },
    #[serde(rename_all = "camelCase")]
    Split {
        id: Id,
        direction: SplitDirection,
        ratio: f64,
        children: Vec<LayoutNode>,
    },
}

impl LayoutNode {
    pub fn leaf(pane_id: impl Into<Id>) -> Self {
        LayoutNode::Leaf {
            pane_id: pane_id.into(),
        }
    }

    /// Todos os pane ids referenciados, da esquerda p/ direita / cima p/ baixo.
    pub fn leaves(&self) -> Vec<Id> {
        match self {
            LayoutNode::Leaf { pane_id } => vec![pane_id.clone()],
            LayoutNode::Split { children, .. } => {
                children.iter().flat_map(|c| c.leaves()).collect()
            }
        }
    }

    pub fn first_leaf(&self) -> Option<Id> {
        self.leaves().into_iter().next()
    }

    /// Substitui a folha de `pane_id` por `node` (usado ao dividir um pane).
    pub fn replacing_leaf(&self, pane_id: &str, node: &LayoutNode) -> LayoutNode {
        match self {
            LayoutNode::Leaf { pane_id: id } => {
                if id == pane_id {
                    node.clone()
                } else {
                    self.clone()
                }
            }
            LayoutNode::Split {
                id,
                direction,
                ratio,
                children,
            } => LayoutNode::Split {
                id: id.clone(),
                direction: *direction,
                ratio: *ratio,
                children: children
                    .iter()
                    .map(|c| c.replacing_leaf(pane_id, node))
                    .collect(),
            },
        }
    }

    /// Remove a folha de `pane_id`, colapsando splits que ficarem com 1 filho.
    /// `None` se a árvore inteira esvaziar.
    pub fn removing_leaf(&self, pane_id: &str) -> Option<LayoutNode> {
        match self {
            LayoutNode::Leaf { pane_id: id } => {
                if id == pane_id {
                    None
                } else {
                    Some(self.clone())
                }
            }
            LayoutNode::Split {
                id,
                direction,
                ratio,
                children,
            } => {
                let survivors: Vec<LayoutNode> =
                    children.iter().filter_map(|c| c.removing_leaf(pane_id)).collect();
                match survivors.len() {
                    0 => None,
                    1 => Some(survivors.into_iter().next().unwrap()),
                    _ => Some(LayoutNode::Split {
                        id: id.clone(),
                        direction: *direction,
                        ratio: *ratio,
                        children: survivors,
                    }),
                }
            }
        }
    }

    /// Ajusta o `ratio` do split de id `split_id` (debounced ao arrastar).
    pub fn setting_ratio(&self, ratio: f64, split_id: &str) -> LayoutNode {
        match self {
            LayoutNode::Leaf { .. } => self.clone(),
            LayoutNode::Split {
                id,
                direction,
                ratio: r,
                children,
            } => LayoutNode::Split {
                id: id.clone(),
                direction: *direction,
                ratio: if id == split_id { ratio } else { *r },
                children: children
                    .iter()
                    .map(|c| c.setting_ratio(ratio, split_id))
                    .collect(),
            },
        }
    }
}

/// Timestamp unix em milissegundos (para created_at/updated_at).
pub fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Manifest {
    /// Manifest inicial de primeira execução: 1 workspace, 1 aba, 1 pane shell.
    pub fn bootstrap(cwd: &str) -> Self {
        let pane_id = crate::new_id("pane");
        let pane = Pane {
            id: pane_id.clone(),
            kind: PaneKind::Terminal,
            tool_profile_id: "shell".to_string(),
            working_directory: cwd.to_string(),
            harness_session_id: None,
            resume_existing: false,
            scrollback_file: None,
            created_at: now_millis(),
            updated_at: now_millis(),
        };
        let tab = Tab {
            id: crate::new_id("tab"),
            folder_id: None,
            title: "shell".to_string(),
            panes: vec![pane],
            layout: LayoutNode::leaf(pane_id.clone()),
            active_pane_id: Some(pane_id),
            created_at: now_millis(),
            updated_at: now_millis(),
        };
        let ws_id = crate::new_id("ws");
        let ws = Workspace {
            id: ws_id.clone(),
            name: "Perene".to_string(),
            order: 0,
            folders: Vec::new(),
            tabs: vec![tab.clone()],
            active_tab_id: Some(tab.id.clone()),
            directory: Some(cwd.to_string()),
        };
        Self {
            version: crate::MANIFEST_VERSION,
            active_workspace_id: Some(ws_id),
            workspaces: vec![ws],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trips_through_json() {
        let m = Manifest::bootstrap("/tmp/proj");
        let json = serde_json::to_string_pretty(&m).unwrap();
        let back: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn manifest_wire_is_camel_case() {
        let m = Manifest::bootstrap("/tmp/proj");
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"activeWorkspaceId\""));
        assert!(json.contains("\"toolProfileId\""));
        assert!(json.contains("\"workingDirectory\""));
    }

    #[test]
    fn layout_node_split_tagged() {
        let node = LayoutNode::Split {
            id: "split_1".into(),
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            children: vec![LayoutNode::leaf("pane_a"), LayoutNode::leaf("pane_b")],
        };
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"type\":\"split\""));
        assert!(json.contains("\"type\":\"leaf\""));
        // O campo da folha DEVE ir como camelCase no wire (o front lê `paneId`).
        assert!(json.contains("\"paneId\":\"pane_a\""), "wire: {json}");
        assert!(!json.contains("pane_id"), "não pode vazar snake_case: {json}");
        let back: LayoutNode = serde_json::from_str(&json).unwrap();
        assert_eq!(node, back);
        assert_eq!(back.leaves(), vec!["pane_a", "pane_b"]);
    }

    #[test]
    fn layout_removing_leaf_collapses_singleton_split() {
        let node = LayoutNode::Split {
            id: "split_1".into(),
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            children: vec![LayoutNode::leaf("pane_a"), LayoutNode::leaf("pane_b")],
        };
        // Removendo b sobra só a → o split colapsa para a folha a.
        let after = node.removing_leaf("pane_b").unwrap();
        assert_eq!(after, LayoutNode::leaf("pane_a"));
        // Removendo o último → árvore vazia.
        assert!(after.removing_leaf("pane_a").is_none());
    }

    #[test]
    fn layout_replacing_and_ratio() {
        let node = LayoutNode::leaf("pane_a");
        let split = LayoutNode::Split {
            id: "split_x".into(),
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            children: vec![LayoutNode::leaf("pane_a"), LayoutNode::leaf("pane_b")],
        };
        let replaced = node.replacing_leaf("pane_a", &split);
        assert_eq!(replaced.leaves(), vec!["pane_a", "pane_b"]);
        let tuned = replaced.setting_ratio(0.7, "split_x");
        if let LayoutNode::Split { ratio, .. } = tuned {
            assert!((ratio - 0.7).abs() < f64::EPSILON);
        } else {
            panic!("esperava split");
        }
    }
}
