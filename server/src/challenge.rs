use async_trait::async_trait;
use brainfuck_chess_engine::{
    ai::BotDifficulty,
    pieces::default_pieces::all_default_definitions,
    rules::{calculate_score_limit, can_piece_be_placed_at_start},
    types::{PieceDefinition, Square},
};
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::database::DataSchema;
use crate::time_control::TimeControlId;
use crate::{DeckPieceRef, PlayerDeckSpec, StartingPieceSpec};

#[derive(Clone, Debug)]
pub(crate) struct OfficialPlacement {
    pub(crate) piece_type: &'static str,
    /// Coordinates are authored from White's side and mirrored for the bot.
    pub(crate) square: Square,
}

#[derive(Clone, Debug)]
pub(crate) struct OfficialPocket {
    pub(crate) piece_type: &'static str,
    pub(crate) count: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ChallengeDefinition {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) board_size: i32,
    pub(crate) opponent_starting: Vec<OfficialPlacement>,
    pub(crate) opponent_pocket: Vec<OfficialPocket>,
    pub(crate) bot_difficulty: BotDifficulty,
    pub(crate) time_control: TimeControlId,
    pub(crate) enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ChallengeSummary {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) board_size: i32,
    pub(crate) map_id: String,
    pub(crate) bot_difficulty: BotDifficulty,
    pub(crate) time_control: TimeControlId,
    pub(crate) cleared: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ChallengeGameMetadata {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) player_id: String,
    pub(crate) bot_player_id: String,
    pub(crate) bot_difficulty: BotDifficulty,
}

#[derive(Clone, Debug)]
pub(crate) struct ChallengeGameContext {
    pub(crate) metadata: ChallengeGameMetadata,
    pub(crate) registered_user_id: Option<String>,
}

fn placement(piece_type: &'static str, file: i32, rank: i32) -> OfficialPlacement {
    OfficialPlacement {
        piece_type,
        square: Square::new(file, rank),
    }
}

pub(crate) fn definitions() -> Vec<ChallengeDefinition> {
    let mut tempest_horde = vec![placement("king", 5, 0)];
    tempest_horde.extend((0..12).map(|file| placement("tempest-pawn", file, 2)));

    let mut tempest_set = vec![
        placement("tempest-rook", 1, 0),
        placement("tempest-knight", 2, 0),
        placement("tempest-bishop", 3, 0),
        placement("tempest-queen", 4, 0),
        placement("king", 5, 0),
        placement("tempest-bishop", 6, 0),
        placement("tempest-knight", 7, 0),
        placement("tempest-rook", 8, 0),
    ];
    tempest_set.extend((1..=8).map(|file| placement("tempest-pawn", file, 2)));

    vec![
        ChallengeDefinition {
            id: "tempest_horde",
            name: "템페스트 호드",
            description: "시작 진영과 포켓을 템페스트 폰으로 채운 물량형 봇에 맞서세요.",
            board_size: 12,
            opponent_starting: tempest_horde,
            opponent_pocket: vec![OfficialPocket {
                piece_type: "tempest-pawn",
                count: 47,
            }],
            bot_difficulty: BotDifficulty::Normal,
            time_control: TimeControlId::Unlimited,
            enabled: true,
        },
        ChallengeDefinition {
            id: "raining_men",
            name: "사람비가 내려와",
            description:
                "구행을 교두보로 삼아 포켓의 공수부대 대원을 지속적으로 투입하는 봇입니다.",
            board_size: 12,
            opponent_starting: vec![placement("king", 5, 0), placement("guhang", 6, 0)],
            opponent_pocket: vec![OfficialPocket {
                piece_type: "paratrooper",
                count: 31,
            }],
            bot_difficulty: BotDifficulty::Normal,
            time_control: TimeControlId::Unlimited,
            enabled: true,
        },
        ChallengeDefinition {
            id: "tempest_set",
            name: "템페스트 셋",
            description: "표준 체스의 주요 기물을 실제 템페스트 계열 기물로 바꾼 테마 덱입니다.",
            board_size: 10,
            opponent_starting: tempest_set,
            opponent_pocket: vec![],
            bot_difficulty: BotDifficulty::Hard,
            time_control: TimeControlId::Unlimited,
            enabled: true,
        },
    ]
}

pub(crate) fn find(id: &str) -> Option<ChallengeDefinition> {
    definitions()
        .into_iter()
        .find(|definition| definition.enabled && definition.id == id)
}

pub(crate) fn validate_registry(definitions: &[ChallengeDefinition]) -> Result<(), String> {
    let catalog = all_default_definitions()
        .into_iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();
    let mut ids = HashSet::new();
    for definition in definitions {
        if definition.id.is_empty() || !ids.insert(definition.id) {
            return Err(format!(
                "Challenge id가 비어 있거나 중복됩니다: {}",
                definition.id
            ));
        }
        if !(8..=12).contains(&definition.board_size) {
            return Err(format!("{}: 지원하지 않는 보드 크기입니다.", definition.id));
        }
        let mut squares = HashSet::new();
        let mut king_count = 0;
        let mut score = 0_u32;
        for entry in &definition.opponent_starting {
            let piece = resolved_definition(&catalog, entry.piece_type)?;
            if entry.square.file < 0
                || entry.square.file >= definition.board_size
                || entry.square.rank < 0
                || entry.square.rank >= definition.board_size
                || !squares.insert(entry.square.to_id())
            {
                return Err(format!("{}: 잘못된 시작 배치입니다.", definition.id));
            }
            if !can_piece_be_placed_at_start(
                piece,
                &"white".into(),
                entry.square,
                definition.board_size,
            ) {
                return Err(format!(
                    "{}: {} 배치 구역이 잘못되었습니다.",
                    definition.id, entry.piece_type
                ));
            }
            king_count += usize::from(piece.is_king);
            if !piece.is_king {
                score = score.saturating_add(piece.score);
            }
        }
        for entry in &definition.opponent_pocket {
            if entry.count == 0 {
                return Err(format!(
                    "{}: 포켓 수량은 1 이상이어야 합니다.",
                    definition.id
                ));
            }
            let piece = resolved_definition(&catalog, entry.piece_type)?;
            if piece.is_king {
                return Err(format!("{}: King은 포켓에 둘 수 없습니다.", definition.id));
            }
            score = score.saturating_add(piece.score.saturating_mul(entry.count));
        }
        if king_count != 1 {
            return Err(format!(
                "{}: 시작 덱에 King이 정확히 1개 필요합니다.",
                definition.id
            ));
        }
        if score > calculate_score_limit(definition.board_size) {
            return Err(format!(
                "{}: 공식 덱 점수가 상한을 초과합니다.",
                definition.id
            ));
        }
    }
    Ok(())
}

fn resolved_definition<'a>(
    catalog: &'a HashMap<String, PieceDefinition>,
    id: &str,
) -> Result<&'a PieceDefinition, String> {
    let resolved = match id {
        "pawn" => "pawn-white",
        "tempest-pawn" => "tempest-pawn-white",
        "bouncing-pawn" => "bouncing-pawn-white",
        "dozer" => "dozer-white",
        "surface-to-air-missile" => "surface-to-air-missile-white",
        other => other,
    };
    catalog
        .get(resolved)
        .ok_or_else(|| format!("존재하지 않는 Challenge 기물 ID입니다: {id}"))
}

pub(crate) fn opponent_deck(definition: &ChallengeDefinition, side: &str) -> PlayerDeckSpec {
    let starting = definition
        .opponent_starting
        .iter()
        .map(|entry| StartingPieceSpec {
            piece: DeckPieceRef::BuiltIn {
                piece_type: entry.piece_type.into(),
            },
            square: if side == "black" {
                Square::new(
                    entry.square.file,
                    definition.board_size - 1 - entry.square.rank,
                )
            } else {
                entry.square
            },
        })
        .collect();
    let pocket = definition
        .opponent_pocket
        .iter()
        .flat_map(|entry| {
            (0..entry.count).map(|_| DeckPieceRef::BuiltIn {
                piece_type: entry.piece_type.into(),
            })
        })
        .collect();
    PlayerDeckSpec {
        name: Some(definition.name.into()),
        starting,
        pocket,
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ChallengeClear {
    pub(crate) challenge_id: String,
    pub(crate) first_cleared_at_ms: i64,
}

#[async_trait]
pub(crate) trait ChallengeProgressRepository: Send + Sync {
    async fn record_clear(
        &self,
        user_id: &str,
        challenge_id: &str,
        cleared_at_ms: i64,
    ) -> Result<(), &'static str>;
    async fn list_clears(&self, user_id: &str) -> Result<Vec<ChallengeClear>, &'static str>;
}

pub(crate) type ChallengeProgressStore = Arc<dyn ChallengeProgressRepository>;

#[derive(Default)]
pub(crate) struct InMemoryChallengeProgressRepository(RwLock<HashMap<(String, String), i64>>);

#[async_trait]
impl ChallengeProgressRepository for InMemoryChallengeProgressRepository {
    async fn record_clear(
        &self,
        user_id: &str,
        challenge_id: &str,
        cleared_at_ms: i64,
    ) -> Result<(), &'static str> {
        self.0
            .write()
            .map_err(|_| "unavailable")?
            .entry((user_id.into(), challenge_id.into()))
            .or_insert(cleared_at_ms);
        Ok(())
    }
    async fn list_clears(&self, user_id: &str) -> Result<Vec<ChallengeClear>, &'static str> {
        Ok(self
            .0
            .read()
            .map_err(|_| "unavailable")?
            .iter()
            .filter(|((owner, _), _)| owner == user_id)
            .map(|((_, challenge_id), cleared)| ChallengeClear {
                challenge_id: challenge_id.clone(),
                first_cleared_at_ms: *cleared,
            })
            .collect())
    }
}

pub(crate) struct PostgresChallengeProgressRepository {
    pool: PgPool,
    table: String,
}

impl PostgresChallengeProgressRepository {
    pub(crate) fn new(pool: PgPool, schema: DataSchema) -> Self {
        Self {
            pool,
            table: format!("{}.challenge_clears", schema.name()),
        }
    }
}

#[async_trait]
impl ChallengeProgressRepository for PostgresChallengeProgressRepository {
    async fn record_clear(
        &self,
        user_id: &str,
        challenge_id: &str,
        cleared_at_ms: i64,
    ) -> Result<(), &'static str> {
        sqlx::query(&format!("INSERT INTO {} (user_id, challenge_id, first_cleared_at_ms) VALUES ($1,$2,$3) ON CONFLICT (user_id, challenge_id) DO NOTHING", self.table))
            .bind(user_id).bind(challenge_id).bind(cleared_at_ms).execute(&self.pool).await.map_err(|_| "unavailable")?;
        Ok(())
    }
    async fn list_clears(&self, user_id: &str) -> Result<Vec<ChallengeClear>, &'static str> {
        let rows = sqlx::query(&format!(
            "SELECT challenge_id, first_cleared_at_ms FROM {} WHERE user_id=$1",
            self.table
        ))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| "unavailable")?;
        rows.into_iter()
            .map(|row| {
                Ok(ChallengeClear {
                    challenge_id: row.try_get("challenge_id").map_err(|_| "unavailable")?,
                    first_cleared_at_ms: row
                        .try_get("first_cleared_at_ms")
                        .map_err(|_| "unavailable")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_registry_contains_the_three_valid_challenges() {
        let registry = definitions();
        assert_eq!(registry.len(), 3);
        validate_registry(&registry).unwrap();
        assert!(registry.iter().any(|entry| entry.id == "tempest_horde"));
        assert!(registry.iter().any(|entry| entry.id == "raining_men"));
        assert!(registry.iter().any(|entry| entry.id == "tempest_set"));
    }

    #[test]
    fn registry_rejects_duplicates_and_unknown_pieces() {
        let mut registry = definitions();
        registry[1].id = registry[0].id;
        assert!(validate_registry(&registry).unwrap_err().contains("중복"));
        let mut registry = definitions();
        registry[0].opponent_starting[0].piece_type = "invented-piece";
        assert!(validate_registry(&registry)
            .unwrap_err()
            .contains("존재하지 않는"));
    }
}
