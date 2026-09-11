use async_trait::async_trait;
use brainfuck_chess_engine::types::{GameState, TurnAction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) draws: Vec<crate::draw::DrawResolution>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AnalysisAppendResult {
    pub(crate) node: AnalysisNode,
    pub(crate) version: i64,
    pub(crate) updated_at_ms: i64,
}

/// Called only after the repository owns the idempotent write/version lock.
/// The parent is transient; persisted nodes contain only the exact outcome.
pub(crate) fn resolve_node(
    node: &mut AnalysisNode,
    parent: Option<&GameState>,
    choose: &mut impl FnMut(usize) -> Result<usize, String>,
) -> Result<(), &'static str> {
    if let Some(parent) = parent {
        let mut next = node.state_after.clone();
        let draws = if let TurnAction::Draw(action) = &node.action {
            let (resolved, draw) = crate::draw::submit(parent.clone(), action.clone(), choose)
                .map_err(|_| "draw_failed")?;
            next = resolved;
            vec![draw]
        } else {
            crate::draw::turn_start(parent, &mut next, choose).map_err(|_| "draw_failed")?
        };
        node.state_hash = state_hash(&next)?;
        node.state_after = normalized_state(next);
        node.draws = draws;
    }
    Ok(())
}

fn resolve_tree(
    tree: &mut AnalysisTree,
    parent: Option<&GameState>,
    choose: &mut impl FnMut(usize) -> Result<usize, String>,
) -> Result<(), &'static str> {
    if let Some(parent) = parent {
        let [node] = tree.nodes.as_mut_slice() else {
            return Err("invalid_parent");
        };
        resolve_node(node, Some(parent), choose)?;
    }
    Ok(())
}

pub(crate) fn normalized_state(mut state: GameState) -> GameState {
    state.history.clear();
    state
}

pub(crate) fn state_hash(state: &GameState) -> Result<String, &'static str> {
    let mut value =
        serde_json::to_value(normalized_state(state.clone())).map_err(|_| "unavailable")?;
    canonicalize_json(&mut value);
    let bytes = serde_json::to_vec(&value).map_err(|_| "unavailable")?;
    Ok(format!("sha256-canonical:{:x}", Sha256::digest(bytes)))
}

pub(crate) fn is_legacy_state_hash(hash: &str) -> bool {
    hash.starts_with("sha256:") && !hash.starts_with("sha256-canonical:")
}

fn canonicalize_json(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(canonicalize_json),
        Value::Object(values) => {
            for child in values.values_mut() {
                canonicalize_json(child);
            }
            let mut entries = std::mem::take(values).into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            values.extend(entries);
        }
        _ => {}
    }
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
        draw_parent: Option<&GameState>,
    ) -> Result<AnalysisTree, &'static str>;
    async fn append(
        &self,
        tree_id: &str,
        owner_user_id: &str,
        node: AnalysisNode,
        expected_version: i64,
        request_id: &str,
        draw_parent: Option<&GameState>,
    ) -> Result<Option<AnalysisAppendResult>, &'static str>;
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

pub(crate) struct InMemoryAnalysisRepository {
    trees: RwLock<HashMap<String, AnalysisTree>>,
    draw_source: Arc<dyn Fn(usize) -> Result<usize, String> + Send + Sync>,
}
impl Default for InMemoryAnalysisRepository {
    fn default() -> Self {
        Self {
            trees: RwLock::default(),
            draw_source: Arc::new(crate::draw::random_index),
        }
    }
}
#[cfg(test)]
impl InMemoryAnalysisRepository {
    pub(crate) fn with_draw_source(
        source: impl Fn(usize) -> Result<usize, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            trees: RwLock::default(),
            draw_source: Arc::new(source),
        }
    }
}

#[async_trait]
impl AnalysisRepository for InMemoryAnalysisRepository {
    async fn list(&self, game_id: &str, owner: &str) -> Result<Vec<AnalysisTree>, &'static str> {
        let mut trees = self
            .trees
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
        mut tree: AnalysisTree,
        _request_id: &str,
        draw_parent: Option<&GameState>,
    ) -> Result<AnalysisTree, &'static str> {
        let mut trees = self.trees.write().map_err(|_| "unavailable")?;
        if let Some(existing) = trees.values().find(|entry| {
            entry.owner_user_id == tree.owner_user_id && entry.request_id == tree.request_id
        }) {
            return Ok(existing.clone());
        }
        resolve_tree(&mut tree, draw_parent, &mut |upper| {
            (self.draw_source)(upper)
        })?;
        trees.insert(tree.id.clone(), tree.clone());
        Ok(tree)
    }
    async fn append(
        &self,
        tree_id: &str,
        owner: &str,
        mut node: AnalysisNode,
        version: i64,
        _request_id: &str,
        draw_parent: Option<&GameState>,
    ) -> Result<Option<AnalysisAppendResult>, &'static str> {
        let mut trees = self.trees.write().map_err(|_| "unavailable")?;
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
            let existing = tree
                .nodes
                .iter()
                .find(|entry| entry.request_id == node.request_id)
                .expect("idempotency match disappeared")
                .clone();
            return Ok(Some(AnalysisAppendResult {
                node: existing,
                version: tree.version,
                updated_at_ms: tree.updated_at_ms,
            }));
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
        resolve_node(&mut node, draw_parent, &mut |upper| {
            (self.draw_source)(upper)
        })?;
        tree.nodes.push(node.clone());
        tree.version += 1;
        tree.updated_at_ms = crate::time_control::now_ms();
        Ok(Some(AnalysisAppendResult {
            node,
            version: tree.version,
            updated_at_ms: tree.updated_at_ms,
        }))
    }
    async fn rename(
        &self,
        tree_id: &str,
        owner: &str,
        name: &str,
        version: i64,
        now: i64,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        let mut trees = self.trees.write().map_err(|_| "unavailable")?;
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
        let mut trees = self.trees.write().map_err(|_| "unavailable")?;
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
        let mut trees = self.trees.write().map_err(|_| "unavailable")?;
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
    #[cfg(test)]
    draw_source: Option<Arc<dyn Fn(usize) -> Result<usize, String> + Send + Sync>>,
}
impl PostgresAnalysisRepository {
    pub(crate) fn new(pool: PgPool, schema: DataSchema) -> Self {
        Self {
            pool,
            trees: schema.table("game_analysis_trees"),
            nodes: schema.table("game_analysis_nodes"),
            #[cfg(test)]
            draw_source: None,
        }
    }
    #[cfg(test)]
    pub(crate) fn with_draw_source(
        mut self,
        source: impl Fn(usize) -> Result<usize, String> + Send + Sync + 'static,
    ) -> Self {
        self.draw_source = Some(Arc::new(source));
        self
    }
    async fn get_tree(
        &self,
        tree_id: &str,
        owner: &str,
    ) -> Result<Option<AnalysisTree>, &'static str> {
        let mut trees = self
            .list_by_clause(
                "trees.id=$1 AND trees.owner_user_id=$2",
                tree_id,
                owner,
                &self.pool,
            )
            .await?;
        Ok(trees.pop())
    }
    async fn get_append_result(
        &self,
        tree_id: &str,
        owner: &str,
        request_id: &str,
    ) -> Result<Option<AnalysisAppendResult>, &'static str> {
        let row = sqlx::query(&format!(
            "SELECT trees.version, trees.updated_at_ms, nodes.id, nodes.parent_node_id, nodes.action, nodes.draws, nodes.state_after, nodes.state_hash, nodes.request_id, nodes.created_at_ms FROM {} nodes JOIN {} trees ON trees.id=nodes.analysis_tree_id WHERE nodes.analysis_tree_id=$1 AND nodes.request_id=$2 AND trees.owner_user_id=$3",
            self.nodes, self.trees
        ))
        .bind(tree_id)
        .bind(request_id)
        .bind(owner)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| "unavailable")?;
        row.map(|row| {
            Ok(AnalysisAppendResult {
                node: AnalysisNode {
                    id: row.try_get("id").map_err(|_| "unavailable")?,
                    parent_node_id: row.try_get("parent_node_id").map_err(|_| "unavailable")?,
                    action: serde_json::from_value(
                        row.try_get("action").map_err(|_| "unavailable")?,
                    )
                    .map_err(|_| "unavailable")?,
                    draws: serde_json::from_value(row.try_get("draws").map_err(|_| "unavailable")?)
                        .map_err(|_| "unavailable")?,
                    state_after: serde_json::from_value(
                        row.try_get("state_after").map_err(|_| "unavailable")?,
                    )
                    .map_err(|_| "unavailable")?,
                    state_hash: row.try_get("state_hash").map_err(|_| "unavailable")?,
                    request_id: row.try_get("request_id").map_err(|_| "unavailable")?,
                    created_at_ms: row.try_get("created_at_ms").map_err(|_| "unavailable")?,
                },
                version: row.try_get("version").map_err(|_| "unavailable")?,
                updated_at_ms: row.try_get("updated_at_ms").map_err(|_| "unavailable")?,
            })
        })
        .transpose()
    }
    async fn list_by_clause<'e>(
        &self,
        clause: &str,
        first: &str,
        owner: &str,
        executor: impl sqlx::PgExecutor<'e>,
    ) -> Result<Vec<AnalysisTree>, &'static str> {
        let rows = sqlx::query(&format!("SELECT trees.id, trees.game_id, trees.owner_user_id, trees.name, trees.base_ply, trees.version, trees.request_id AS tree_request_id, trees.created_at_ms, trees.updated_at_ms, nodes.id AS node_id, nodes.parent_node_id, nodes.action, nodes.draws, nodes.state_after, nodes.state_hash, nodes.request_id AS node_request_id, nodes.created_at_ms AS node_created_at_ms FROM {} trees LEFT JOIN {} nodes ON nodes.analysis_tree_id=trees.id WHERE {} ORDER BY trees.created_at_ms, nodes.created_at_ms", self.trees, self.nodes, clause)).bind(first).bind(owner).fetch_all(executor).await.map_err(|_| "unavailable")?;
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
                    draws: serde_json::from_value(row.try_get("draws").map_err(|_| "unavailable")?)
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
        self.list_by_clause(
            "trees.game_id=$1 AND trees.owner_user_id=$2",
            game,
            owner,
            &self.pool,
        )
        .await
    }
    async fn create(
        &self,
        mut tree: AnalysisTree,
        request_id: &str,
        draw_parent: Option<&GameState>,
    ) -> Result<AnalysisTree, &'static str> {
        let mut tx = self.pool.begin().await.map_err(|_| "unavailable")?;
        // JSON tuple encoding separates keys unambiguously, including schemas.
        // A 64-bit hash collision can only serialize unrelated writes: identity
        // is always checked using the full owner/request pair, never the hash.
        let lock_key = serde_json::to_string(&[
            "analysis-create-v1",
            &self.trees,
            &tree.owner_user_id,
            request_id,
        ])
        .map_err(|_| "unavailable")?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(lock_key)
            .execute(&mut *tx)
            .await
            .map_err(|_| "unavailable")?;
        // Use the transaction connection for reload too (even a one-slot pool).
        if let Some(existing) = self
            .list_by_clause(
                "trees.request_id=$1 AND trees.owner_user_id=$2",
                request_id,
                &tree.owner_user_id,
                &mut *tx,
            )
            .await?
            .pop()
        {
            tx.commit().await.map_err(|_| "unavailable")?;
            return Ok(existing);
        }
        let inserted = sqlx::query(&format!("INSERT INTO {} (id,game_id,owner_user_id,name,base_ply,version,request_id,created_at_ms,updated_at_ms) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (owner_user_id,request_id) DO NOTHING", self.trees)).bind(&tree.id).bind(&tree.game_id).bind(&tree.owner_user_id).bind(&tree.name).bind(tree.base_ply as i32).bind(tree.version).bind(request_id).bind(tree.created_at_ms).bind(tree.updated_at_ms).execute(&mut *tx).await.map_err(|error| {
            if error.as_database_error().and_then(|error| error.constraint()) == Some("game_analysis_trees_pkey") {
                "conflict"
            } else {
                "unavailable"
            }
        })?.rows_affected();
        if inserted > 0 {
            resolve_tree(&mut tree, draw_parent, &mut |upper| {
                #[cfg(test)]
                if let Some(source) = &self.draw_source {
                    return source(upper);
                }
                crate::draw::random_index(upper)
            })?;
            for node in &tree.nodes {
                sqlx::query(&format!("INSERT INTO {} (id,analysis_tree_id,parent_node_id,action,state_after,state_hash,request_id,created_at_ms,draws) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)", self.nodes)).bind(&node.id).bind(&tree.id).bind(node.parent_node_id.as_deref()).bind(serde_json::to_value(&node.action).map_err(|_| "unavailable")?).bind(serde_json::to_value(&node.state_after).map_err(|_| "unavailable")?).bind(&node.state_hash).bind(request_id).bind(node.created_at_ms).bind(serde_json::to_value(&node.draws).map_err(|_| "unavailable")?).execute(&mut *tx).await.map_err(|_| "unavailable")?;
            }
        }
        if inserted > 0 {
            tx.commit().await.map_err(|_| "unavailable")?;
            Ok(tree)
        } else {
            // Retain the exact-key fallback for writers from older revisions
            // which do not yet participate in the advisory-lock protocol.
            let existing = self
                .list_by_clause(
                    "trees.request_id=$1 AND trees.owner_user_id=$2",
                    request_id,
                    &tree.owner_user_id,
                    &mut *tx,
                )
                .await?
                .pop()
                .ok_or("unavailable")?;
            tx.commit().await.map_err(|_| "unavailable")?;
            Ok(existing)
        }
    }
    async fn append(
        &self,
        tree_id: &str,
        owner: &str,
        mut node: AnalysisNode,
        version: i64,
        request_id: &str,
        draw_parent: Option<&GameState>,
    ) -> Result<Option<AnalysisAppendResult>, &'static str> {
        if let Some(existing) = self.get_append_result(tree_id, owner, request_id).await? {
            return Ok(Some(existing));
        }
        let mut tx = self.pool.begin().await.map_err(|_| "unavailable")?;
        if draw_parent.is_some() {
            let locked = sqlx::query(&format!(
                "SELECT id FROM {} WHERE id=$1 AND owner_user_id=$2 FOR UPDATE",
                self.trees
            ))
            .bind(tree_id)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| "unavailable")?;
            if locked.is_none() {
                return Ok(None);
            }
            let duplicate = sqlx::query(&format!(
                "SELECT id FROM {} WHERE analysis_tree_id=$1 AND request_id=$2",
                self.nodes
            ))
            .bind(tree_id)
            .bind(request_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| "unavailable")?;
            if duplicate.is_some() {
                tx.rollback().await.map_err(|_| "unavailable")?;
                return self.get_append_result(tree_id, owner, request_id).await;
            }
        }
        let updated=sqlx::query(&format!("UPDATE {} SET version=version+1,updated_at_ms=$4 WHERE id=$1 AND owner_user_id=$2 AND version=$3",self.trees)).bind(tree_id).bind(owner).bind(version).bind(node.created_at_ms).execute(&mut *tx).await.map_err(|_| "unavailable")?.rows_affected();
        if updated == 0 {
            tx.rollback().await.map_err(|_| "unavailable")?;
            if let Some(existing) = self.get_append_result(tree_id, owner, request_id).await? {
                return Ok(Some(existing));
            }
            return if self.get_tree(tree_id, owner).await?.is_some() {
                Err("conflict")
            } else {
                Ok(None)
            };
        }
        resolve_node(&mut node, draw_parent, &mut crate::draw::random_index)?;
        sqlx::query(&format!("INSERT INTO {} (id,analysis_tree_id,parent_node_id,action,state_after,state_hash,request_id,created_at_ms,draws) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (analysis_tree_id,request_id) DO NOTHING",self.nodes)).bind(&node.id).bind(tree_id).bind(node.parent_node_id.as_deref()).bind(serde_json::to_value(&node.action).map_err(|_| "unavailable")?).bind(serde_json::to_value(&node.state_after).map_err(|_| "unavailable")?).bind(&node.state_hash).bind(request_id).bind(node.created_at_ms).bind(serde_json::to_value(&node.draws).map_err(|_| "unavailable")?).execute(&mut *tx).await.map_err(|_| "invalid_parent")?;
        tx.commit().await.map_err(|_| "unavailable")?;
        self.get_append_result(tree_id, owner, request_id).await
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
        draws: Vec::new(),
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
            ruleset: Default::default(),
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
            draws: Vec::new(),
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
        let tree = repository.create(tree, "create", None).await.unwrap();
        let tree_id = tree.id.clone();
        let first = repository
            .append(
                &tree_id,
                "owner",
                node("a", Some(&root), "a"),
                tree.version,
                "a",
                None,
            )
            .await
            .unwrap()
            .unwrap();
        let second = repository
            .append(
                &tree_id,
                "owner",
                node("b", Some(&root), "b"),
                first.version,
                "b",
                None,
            )
            .await
            .unwrap()
            .unwrap();
        let third = repository
            .append(
                &tree_id,
                "owner",
                node("a-child", Some("a"), "a-child"),
                second.version,
                "a-child",
                None,
            )
            .await
            .unwrap()
            .unwrap();
        let tree = repository
            .delete_subtree(&tree_id, "owner", "a", third.version, 4)
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
                None,
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
                None,
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
                None,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retried.node.id, updated.node.id);
        assert_eq!(retried.version, updated.version);
        assert_eq!(
            repository
                .append(
                    &tree.id,
                    "owner",
                    node("stale", Some(&root), "stale"),
                    tree.version,
                    "stale",
                    None,
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

    #[test]
    fn state_hash_is_independent_of_hash_map_insertion_order() {
        let mut left = state();
        left.global_state.insert("alpha".into(), 1);
        left.global_state.insert("beta".into(), 2);
        let mut right = state();
        right.global_state.insert("beta".into(), 2);
        right.global_state.insert("alpha".into(), 1);
        assert_eq!(state_hash(&left).unwrap(), state_hash(&right).unwrap());
    }
    #[test]
    fn ruleset_preserves_pre_g1_state_json_and_canonical_hash() {
        let old_json: serde_json::Value = serde_json::from_str(r#"{"id":"game","board":{"size":8,"squares":{}},"pieces":{},"piece_definitions":{},"players":{},"current_player":"white","turn_number":1,"phase":"playing","en_passant_target":null,"en_passant_available_to":null,"global_state":{},"history":[],"result":null}"#).unwrap();
        let restored: GameState = serde_json::from_value(old_json.clone()).unwrap();
        assert_eq!(
            restored.ruleset,
            brainfuck_chess_engine::types::DeckRuleset::Legacy
        );
        assert_eq!(serde_json::to_value(&restored).unwrap(), old_json);
        assert_eq!(
            state_hash(&restored).unwrap(),
            "sha256-canonical:35bef86d93aadeb18fdd1c7a9a5525ec936f3ea70085de74b3e8311fdd1fe525"
        );
        let mut explicit = old_json.clone();
        explicit["ruleset"] = serde_json::json!("legacy");
        let explicit: GameState = serde_json::from_value(explicit).unwrap();
        assert_eq!(
            state_hash(&explicit).unwrap(),
            state_hash(&restored).unwrap()
        );
        let mut standard = restored.clone();
        standard.ruleset = brainfuck_chess_engine::types::DeckRuleset::Standard;
        assert_ne!(
            state_hash(&standard).unwrap(),
            state_hash(&restored).unwrap()
        );
        for unknown in [
            serde_json::json!("future"),
            serde_json::Value::Null,
            serde_json::json!(2),
        ] {
            let mut invalid = old_json.clone();
            invalid["ruleset"] = unknown;
            assert!(serde_json::from_value::<GameState>(invalid).is_err());
        }
    }
}
