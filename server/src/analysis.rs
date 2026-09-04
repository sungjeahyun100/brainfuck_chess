use async_trait::async_trait;
use brainfuck_chess_engine::types::{GameState, TurnAction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

use crate::database::DataSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AnalysisNode {
    pub(crate) id: String,
    pub(crate) parent_node_id: Option<String>,
    pub(crate) action: TurnAction,
    pub(crate) state_after: GameState,
    pub(crate) state_hash: String,
    pub(crate) created_at_ms: i64,
    #[serde(skip_serializing)]
    pub(crate) request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AnalysisTree {
    pub(crate) id: String,
    pub(crate) game_id: String,
    #[serde(skip_serializing)]
    pub(crate) owner_user_id: String,
    pub(crate) name: String,
    pub(crate) base_ply: u32,
    pub(crate) version: i64,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
    pub(crate) nodes: Vec<AnalysisNode>,
    #[serde(skip_serializing)]
    pub(crate) request_id: String,
}

pub(crate) fn normalized_state(mut state: GameState) -> GameState {
    state.history.clear();
    state
}

pub(crate) fn state_hash(state: &GameState) -> Result<String, &'static str> {
    let bytes = serde_json::to_vec(&normalized_state(state.clone())).map_err(|_| "unavailable")?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

#[async_trait]
pub(crate) trait AnalysisRepository: Send + Sync {
    async fn list(
        &self,
        game_id: &str,
        owner_user_id: &str,
    ) -> Result<Vec<AnalysisTree>, &'static str>;
    async fn create(
        &self,
        tree: AnalysisTree,
        request_id: &str,
    ) -> Result<AnalysisTree, &'static str>;
    async fn append(
        &self,
        tree_id: &str,
        owner_user_id: &str,
        node: AnalysisNode,
        expected_version: i64,
        request_id: &str,
    ) -> Result<Option<AnalysisTree>, &'static str>;
    async fn rename(
        &self,
        tree_id: &str,
        owner_user_id: &str,
        name: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<Option<AnalysisTree>, &'static str>;
    async fn delete_tree(&self, tree_id: &str, owner_user_id: &str) -> Result<bool, &'static str>;
    async fn delete_subtree(
        &self,
        tree_id: &str,
        owner_user_id: &str,
        node_id: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<Option<AnalysisTree>, &'static str>;
}

pub(crate) type AnalysisStore = Arc<dyn AnalysisRepository>;

#[derive(Default)]
pub(crate) struct InMemoryAnalysisRepository(RwLock<HashMap<String, AnalysisTree>>);

#[async_trait]
impl AnalysisRepository for InMemoryAnalysisRepository {
    async fn list(&self, game_id: &str, owner: &str) -> Result<Vec<AnalysisTree>, &'static str> {
        let mut trees = self
            .0
            .read()
            .map_err(|_| "unavailable")?
            .values()
            .filter(|tree| tree.game_id == game_id && tree.owner_user_id == owner)
            .cloned()
            .collect::<Vec<_>>();
        trees.sort_by_key(|tree| tree.created_at_ms);
        Ok(trees)
    }
    async fn create(
        &self,
        tree: AnalysisTree,
        _request_id: &str,
    ) -> Result<AnalysisTree, &'static str> {
        let mut trees = self.0.write().map_err(|_| "unavailable")?;
        if let Some(existing) = trees.values().find(|entry| {
            entry.owner_user_id == tree.owner_user_id && entry.request_id == tree.request_id
        }) {
            return Ok(existing.clone());
        }
        trees.insert(tree.id.clone(), tree.clone());
        Ok(tree)
    }
    async fn append(
        &self,
        tree_id: &str,
        owner: &str,
        node: AnalysisNode,
        version: i64,
        _request_id: &str,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        let mut trees = self.0.write().map_err(|_| "unavailable")?;
        let Some(tree) = trees.get_mut(tree_id) else {
            return Ok(None);
        };
        if tree.owner_user_id != owner {
            return Err("forbidden");
        }
        if tree
            .nodes
            .iter()
            .any(|entry| entry.request_id == node.request_id)
        {
            return Ok(Some(tree.clone()));
        }
        if tree.version != version {
            return Err("conflict");
        }
        if node
            .parent_node_id
            .as_ref()
            .is_some_and(|id| !tree.nodes.iter().any(|entry| &entry.id == id))
        {
            return Err("invalid_parent");
        }
        tree.nodes.push(node);
        tree.version += 1;
        tree.updated_at_ms = crate::time_control::now_ms();
        Ok(Some(tree.clone()))
    }
    async fn rename(
        &self,
        tree_id: &str,
        owner: &str,
        name: &str,
        version: i64,
        now: i64,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        let mut trees = self.0.write().map_err(|_| "unavailable")?;
        let Some(tree) = trees.get_mut(tree_id) else {
            return Ok(None);
        };
        if tree.owner_user_id != owner {
            return Err("forbidden");
        }
        if tree.version != version {
            return Err("conflict");
        }
        tree.name = name.into();
        tree.version += 1;
        tree.updated_at_ms = now;
        Ok(Some(tree.clone()))
    }
    async fn delete_tree(&self, tree_id: &str, owner: &str) -> Result<bool, &'static str> {
        let mut trees = self.0.write().map_err(|_| "unavailable")?;
        if trees
            .get(tree_id)
            .is_some_and(|tree| tree.owner_user_id != owner)
        {
            return Err("forbidden");
        }
        Ok(trees.remove(tree_id).is_some())
    }
    async fn delete_subtree(
        &self,
        tree_id: &str,
        owner: &str,
        node_id: &str,
        version: i64,
        now: i64,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        let mut trees = self.0.write().map_err(|_| "unavailable")?;
        let Some(tree) = trees.get_mut(tree_id) else {
            return Ok(None);
        };
        if tree.owner_user_id != owner {
            return Err("forbidden");
        }
        if tree.version != version {
            return Err("conflict");
        }
        if !tree.nodes.iter().any(|node| node.id == node_id) {
            return Ok(None);
        }
        let mut removed = vec![node_id.to_owned()];
        loop {
            let children = tree
                .nodes
                .iter()
                .filter(|node| {
                    node.parent_node_id
                        .as_ref()
                        .is_some_and(|parent| removed.contains(parent))
                        && !removed.contains(&node.id)
                })
                .map(|node| node.id.clone())
                .collect::<Vec<_>>();
            if children.is_empty() {
                break;
            }
            removed.extend(children);
        }
        tree.nodes.retain(|node| !removed.contains(&node.id));
        tree.version += 1;
        tree.updated_at_ms = now;
        Ok(Some(tree.clone()))
    }
}

pub(crate) struct PostgresAnalysisRepository {
    pool: PgPool,
    trees: String,
    nodes: String,
}
impl PostgresAnalysisRepository {
    pub(crate) fn new(pool: PgPool, schema: DataSchema) -> Self {
        Self {
            pool,
            trees: schema.table("game_analysis_trees"),
            nodes: schema.table("game_analysis_nodes"),
        }
    }
    async fn get_tree(
        &self,
        tree_id: &str,
        owner: &str,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        let mut trees = self
            .list_by_clause("trees.id=$1 AND trees.owner_user_id=$2", tree_id, owner)
            .await?;
        Ok(trees.pop())
    }
    async fn list_by_clause(
        &self,
        clause: &str,
        first: &str,
        owner: &str,
    ) -> Result<Vec<AnalysisTree>, &'static str> {
        let rows = sqlx::query(&format!("SELECT trees.id, trees.game_id, trees.owner_user_id, trees.name, trees.base_ply, trees.version, trees.request_id AS tree_request_id, trees.created_at_ms, trees.updated_at_ms, nodes.id AS node_id, nodes.parent_node_id, nodes.action, nodes.state_after, nodes.state_hash, nodes.request_id AS node_request_id, nodes.created_at_ms AS node_created_at_ms FROM {} trees LEFT JOIN {} nodes ON nodes.analysis_tree_id=trees.id WHERE {} ORDER BY trees.created_at_ms, nodes.created_at_ms", self.trees, self.nodes, clause)).bind(first).bind(owner).fetch_all(&self.pool).await.map_err(|_| "unavailable")?;
        let mut output: Vec<AnalysisTree> = Vec::new();
        for row in rows {
            let id: String = row.try_get("id").map_err(|_| "unavailable")?;
            let index = output.iter().position(|tree| tree.id == id);
            let idx = if let Some(index) = index {
                index
            } else {
                output.push(AnalysisTree {
                    id: id.clone(),
                    game_id: row.try_get("game_id").map_err(|_| "unavailable")?,
                    owner_user_id: row.try_get("owner_user_id").map_err(|_| "unavailable")?,
                    name: row.try_get("name").map_err(|_| "unavailable")?,
                    base_ply: row
                        .try_get::<i32, _>("base_ply")
                        .map_err(|_| "unavailable")? as u32,
                    version: row.try_get("version").map_err(|_| "unavailable")?,
                    created_at_ms: row.try_get("created_at_ms").map_err(|_| "unavailable")?,
                    updated_at_ms: row.try_get("updated_at_ms").map_err(|_| "unavailable")?,
                    nodes: vec![],
                    request_id: row.try_get("tree_request_id").map_err(|_| "unavailable")?,
                });
                output.len() - 1
            };
            if let Some(node_id) = row
                .try_get::<Option<String>, _>("node_id")
                .map_err(|_| "unavailable")?
            {
                output[idx].nodes.push(AnalysisNode {
                    id: node_id,
                    parent_node_id: row.try_get("parent_node_id").map_err(|_| "unavailable")?,
                    action: serde_json::from_value(
                        row.try_get("action").map_err(|_| "unavailable")?,
                    )
                    .map_err(|_| "unavailable")?,
                    state_after: serde_json::from_value(
                        row.try_get("state_after").map_err(|_| "unavailable")?,
                    )
                    .map_err(|_| "unavailable")?,
                    state_hash: row.try_get("state_hash").map_err(|_| "unavailable")?,
                    created_at_ms: row
                        .try_get("node_created_at_ms")
                        .map_err(|_| "unavailable")?,
                    request_id: row.try_get("node_request_id").map_err(|_| "unavailable")?,
                });
            }
        }
        Ok(output)
    }
}

#[async_trait]
impl AnalysisRepository for PostgresAnalysisRepository {
    async fn list(&self, game: &str, owner: &str) -> Result<Vec<AnalysisTree>, &'static str> {
        self.list_by_clause("trees.game_id=$1 AND trees.owner_user_id=$2", game, owner)
            .await
    }
    async fn create(
        &self,
        tree: AnalysisTree,
        request_id: &str,
    ) -> Result<AnalysisTree, &'static str> {
        let mut tx = self.pool.begin().await.map_err(|_| "unavailable")?;
        let inserted = sqlx::query(&format!("INSERT INTO {} (id,game_id,owner_user_id,name,base_ply,version,request_id,created_at_ms,updated_at_ms) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (owner_user_id,request_id) DO NOTHING", self.trees)).bind(&tree.id).bind(&tree.game_id).bind(&tree.owner_user_id).bind(&tree.name).bind(tree.base_ply as i32).bind(tree.version).bind(request_id).bind(tree.created_at_ms).bind(tree.updated_at_ms).execute(&mut *tx).await.map_err(|_| "unavailable")?.rows_affected();
        if inserted > 0 {
            for node in &tree.nodes {
                sqlx::query(&format!("INSERT INTO {} (id,analysis_tree_id,parent_node_id,action,state_after,state_hash,request_id,created_at_ms) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)", self.nodes)).bind(&node.id).bind(&tree.id).bind(node.parent_node_id.as_deref()).bind(serde_json::to_value(&node.action).map_err(|_| "unavailable")?).bind(serde_json::to_value(&node.state_after).map_err(|_| "unavailable")?).bind(&node.state_hash).bind(request_id).bind(node.created_at_ms).execute(&mut *tx).await.map_err(|_| "unavailable")?;
            }
        }
        tx.commit().await.map_err(|_| "unavailable")?;
        if inserted > 0 {
            Ok(tree)
        } else {
            let row = sqlx::query(&format!(
                "SELECT id FROM {} WHERE owner_user_id=$1 AND request_id=$2",
                self.trees
            ))
            .bind(&tree.owner_user_id)
            .bind(request_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|_| "unavailable")?;
            self.get_tree(
                row.try_get::<String, _>("id")
                    .map_err(|_| "unavailable")?
                    .as_str(),
                &tree.owner_user_id,
            )
            .await?
            .ok_or("unavailable")
        }
    }
    async fn append(
        &self,
        tree_id: &str,
        owner: &str,
        node: AnalysisNode,
        version: i64,
        request_id: &str,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        if sqlx::query_scalar::<_, bool>(&format!("SELECT EXISTS(SELECT 1 FROM {} nodes JOIN {} trees ON trees.id=nodes.analysis_tree_id WHERE nodes.analysis_tree_id=$1 AND nodes.request_id=$2 AND trees.owner_user_id=$3)",self.nodes,self.trees)).bind(tree_id).bind(request_id).bind(owner).fetch_one(&self.pool).await.map_err(|_|"unavailable")? { return self.get_tree(tree_id,owner).await }
        let mut tx = self.pool.begin().await.map_err(|_| "unavailable")?;
        let updated=sqlx::query(&format!("UPDATE {} SET version=version+1,updated_at_ms=$4 WHERE id=$1 AND owner_user_id=$2 AND version=$3",self.trees)).bind(tree_id).bind(owner).bind(version).bind(node.created_at_ms).execute(&mut *tx).await.map_err(|_| "unavailable")?.rows_affected();
        if updated == 0 {
            tx.rollback().await.ok();
            return if self.get_tree(tree_id, owner).await?.is_some() {
                Err("conflict")
            } else {
                Ok(None)
            };
        }
        sqlx::query(&format!("INSERT INTO {} (id,analysis_tree_id,parent_node_id,action,state_after,state_hash,request_id,created_at_ms) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (analysis_tree_id,request_id) DO NOTHING",self.nodes)).bind(&node.id).bind(tree_id).bind(node.parent_node_id.as_deref()).bind(serde_json::to_value(&node.action).map_err(|_| "unavailable")?).bind(serde_json::to_value(&node.state_after).map_err(|_| "unavailable")?).bind(&node.state_hash).bind(request_id).bind(node.created_at_ms).execute(&mut *tx).await.map_err(|_| "invalid_parent")?;
        tx.commit().await.map_err(|_| "unavailable")?;
        self.get_tree(tree_id, owner).await
    }
    async fn rename(
        &self,
        tree_id: &str,
        owner: &str,
        name: &str,
        version: i64,
        now: i64,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        let changed=sqlx::query(&format!("UPDATE {} SET name=$4,version=version+1,updated_at_ms=$5 WHERE id=$1 AND owner_user_id=$2 AND version=$3",self.trees)).bind(tree_id).bind(owner).bind(version).bind(name).bind(now).execute(&self.pool).await.map_err(|_|"unavailable")?.rows_affected();
        if changed == 0 {
            return if self.get_tree(tree_id, owner).await?.is_some() {
                Err("conflict")
            } else {
                Ok(None)
            };
        }
        self.get_tree(tree_id, owner).await
    }
    async fn delete_tree(&self, tree_id: &str, owner: &str) -> Result<bool, &'static str> {
        Ok(sqlx::query(&format!(
            "DELETE FROM {} WHERE id=$1 AND owner_user_id=$2",
            self.trees
        ))
        .bind(tree_id)
        .bind(owner)
        .execute(&self.pool)
        .await
        .map_err(|_| "unavailable")?
        .rows_affected()
            > 0)
    }
    async fn delete_subtree(
        &self,
        tree_id: &str,
        owner: &str,
        node_id: &str,
        version: i64,
        now: i64,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        let mut tx = self.pool.begin().await.map_err(|_| "unavailable")?;
        let changed=sqlx::query(&format!("UPDATE {} SET version=version+1,updated_at_ms=$4 WHERE id=$1 AND owner_user_id=$2 AND version=$3",self.trees)).bind(tree_id).bind(owner).bind(version).bind(now).execute(&mut *tx).await.map_err(|_|"unavailable")?.rows_affected();
        if changed == 0 {
            tx.rollback().await.ok();
            return if self.get_tree(tree_id, owner).await?.is_some() {
                Err("conflict")
            } else {
                Ok(None)
            };
        }
        let deleted=sqlx::query(&format!("WITH RECURSIVE descendants AS (SELECT id FROM {nodes} WHERE id=$1 AND analysis_tree_id=$2 UNION ALL SELECT child.id FROM {nodes} child JOIN descendants parent ON child.parent_node_id=parent.id WHERE child.analysis_tree_id=$2) DELETE FROM {nodes} WHERE id IN (SELECT id FROM descendants)",nodes=self.nodes)).bind(node_id).bind(tree_id).execute(&mut *tx).await.map_err(|_|"unavailable")?.rows_affected();
        if deleted == 0 {
            tx.rollback().await.ok();
            return Ok(None);
        }
        tx.commit().await.map_err(|_| "unavailable")?;
        self.get_tree(tree_id, owner).await
    }
}

pub(crate) fn new_tree(
    game_id: String,
    owner_user_id: String,
    name: String,
    base_ply: u32,
    action: TurnAction,
    state_after: GameState,
    now: i64,
    request_id: String,
) -> Result<AnalysisTree, &'static str> {
    let state_after = normalized_state(state_after);
    let node = AnalysisNode {
        id: Uuid::new_v4().to_string(),
        parent_node_id: None,
        action,
        state_hash: state_hash(&state_after)?,
        state_after,
        created_at_ms: now,
        request_id: request_id.clone(),
    };
    Ok(AnalysisTree {
        id: Uuid::new_v4().to_string(),
        game_id,
        owner_user_id,
        name,
        base_ply,
        version: 1,
        created_at_ms: now,
        updated_at_ms: now,
        nodes: vec![node],
        request_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use brainfuck_chess_engine::types::{Board, ChessemblyProgramCache, GamePhase};

    fn state() -> GameState {
        GameState {
            id: "game".into(),
            board: Board {
                size: 8,
                squares: HashMap::new(),
                air_squares: HashMap::new(),
                terrain: HashMap::new(),
            },
            pieces: HashMap::new(),
            piece_definitions: HashMap::new(),
            custom_piece_manifest: vec![],
            players: HashMap::new(),
            current_player: "white".into(),
            turn_number: 1,
            phase: GamePhase::Playing,
            en_passant_target: None,
            en_passant_available_to: None,
            global_state: HashMap::new(),
            history: vec![],
            result: None,
            chessembly_program_cache: ChessemblyProgramCache::default(),
        }
    }
    fn action() -> TurnAction {
        serde_json::from_value(serde_json::json!({"type":"move","player_id":"white","piece_id":"p","from":{"file":0,"rank":0},"to":{"file":0,"rank":1},"move_option_id":"normal","source_layer_ids":[],"effects":{"global_state_updates":[],"piece_state_updates":[],"cooldown_updates":[]}})).unwrap()
    }
    fn node(id: &str, parent: Option<&str>, request: &str) -> AnalysisNode {
        let state = state();
        AnalysisNode {
            id: id.into(),
            parent_node_id: parent.map(Into::into),
            action: action(),
            state_hash: state_hash(&state).unwrap(),
            state_after: state,
            created_at_ms: 2,
            request_id: request.into(),
        }
    }

    #[tokio::test]
    async fn branches_are_independent_and_subtree_delete_is_recursive() {
        let repository = InMemoryAnalysisRepository::default();
        let tree = new_tree(
            "game".into(),
            "owner".into(),
            "Variation 1".into(),
            17,
            action(),
            state(),
            1,
            "create".into(),
        )
        .unwrap();
        let root = tree.nodes[0].id.clone();
        let tree = repository.create(tree, "create").await.unwrap();
        let tree = repository
            .append(
                &tree.id,
                "owner",
                node("a", Some(&root), "a"),
                tree.version,
                "a",
            )
            .await
            .unwrap()
            .unwrap();
        let tree = repository
            .append(
                &tree.id,
                "owner",
                node("b", Some(&root), "b"),
                tree.version,
                "b",
            )
            .await
            .unwrap()
            .unwrap();
        let tree = repository
            .append(
                &tree.id,
                "owner",
                node("a-child", Some("a"), "a-child"),
                tree.version,
                "a-child",
            )
            .await
            .unwrap()
            .unwrap();
        let tree = repository
            .delete_subtree(&tree.id, "owner", "a", tree.version, 4)
            .await
            .unwrap()
            .unwrap();
        assert!(tree.nodes.iter().any(|node| node.id == "b"));
        assert!(!tree
            .nodes
            .iter()
            .any(|node| node.id == "a" || node.id == "a-child"));
    }

    #[tokio::test]
    async fn writes_are_owner_checked_versioned_and_idempotent() {
        let repository = InMemoryAnalysisRepository::default();
        let tree = repository
            .create(
                new_tree(
                    "game".into(),
                    "owner".into(),
                    "V".into(),
                    0,
                    action(),
                    state(),
                    1,
                    "create".into(),
                )
                .unwrap(),
                "create",
            )
            .await
            .unwrap();
        assert_eq!(
            repository
                .rename(&tree.id, "attacker", "x", tree.version, 2)
                .await
                .unwrap_err(),
            "forbidden"
        );
        let root = tree.nodes[0].id.clone();
        let updated = repository
            .append(
                &tree.id,
                "owner",
                node("one", Some(&root), "retry"),
                tree.version,
                "retry",
            )
            .await
            .unwrap()
            .unwrap();
        let retried = repository
            .append(
                &tree.id,
                "owner",
                node("two", Some(&root), "retry"),
                tree.version,
                "retry",
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retried.nodes.len(), updated.nodes.len());
        assert_eq!(
            repository
                .append(
                    &tree.id,
                    "owner",
                    node("stale", Some(&root), "stale"),
                    tree.version,
                    "stale"
                )
                .await
                .unwrap_err(),
            "conflict"
        );
    }

    #[test]
    fn state_hash_detects_tampering() {
        let original = state();
        let mut tampered = original.clone();
        tampered.turn_number = 2;
        assert_ne!(
            state_hash(&original).unwrap(),
            state_hash(&tampered).unwrap()
        );
    }
}
