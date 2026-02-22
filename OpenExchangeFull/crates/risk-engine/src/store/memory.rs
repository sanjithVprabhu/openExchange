use crate::error::RiskError;
use crate::store::traits::{RiskResult, RiskStore};
use crate::types::{Position, UserRiskState};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

pub struct InMemoryRiskStore {
    user_states: RwLock<HashMap<Uuid, UserRiskState>>,
    positions: RwLock<HashMap<(Uuid, String), Position>>,
}

impl InMemoryRiskStore {
    pub fn new() -> Self {
        Self {
            user_states: RwLock::new(HashMap::new()),
            positions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryRiskStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RiskStore for InMemoryRiskStore {
    async fn get_user_state(&self, user_id: Uuid) -> RiskResult<Option<UserRiskState>> {
        let states = self.user_states.read().unwrap();
        Ok(states.get(&user_id).cloned())
    }

    async fn save_user_state(&self, state: &UserRiskState) -> RiskResult<()> {
        let mut states = self.user_states.write().unwrap();
        states.insert(state.user_id, state.clone());
        Ok(())
    }

    async fn get_position(&self, user_id: Uuid, instrument_id: &str) -> RiskResult<Option<Position>> {
        let positions = self.positions.read().unwrap();
        Ok(positions.get(&(user_id, instrument_id.to_string())).cloned())
    }

    async fn save_position(&self, position: &Position) -> RiskResult<()> {
        let mut positions = self.positions.write().unwrap();
        positions.insert(
            (position.user_id, position.instrument_id.clone()),
            position.clone(),
        );
        Ok(())
    }

    async fn delete_position(&self, user_id: Uuid, instrument_id: &str) -> RiskResult<()> {
        let mut positions = self.positions.write().unwrap();
        positions.remove(&(user_id, instrument_id.to_string()));
        Ok(())
    }

    async fn list_positions(&self, user_id: Uuid) -> RiskResult<Vec<Position>> {
        let positions = self.positions.read().unwrap();
        let result: Vec<Position> = positions
            .iter()
            .filter(|((uid, _), _)| *uid == user_id)
            .map(|((_, _), pos)| pos.clone())
            .collect();
        Ok(result)
    }
}
