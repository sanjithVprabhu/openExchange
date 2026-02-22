use async_trait::async_trait;
use crate::types::{Position, UserRiskState};
use uuid::Uuid;

pub type RiskResult<T> = std::result::Result<T, crate::error::RiskError>;

#[async_trait]
pub trait RiskStore: Send + Sync {
    async fn get_user_state(&self, user_id: Uuid) -> RiskResult<Option<UserRiskState>>;
    
    async fn save_user_state(&self, state: &UserRiskState) -> RiskResult<()>;
    
    async fn get_position(&self, user_id: Uuid, instrument_id: &str) -> RiskResult<Option<Position>>;
    
    async fn save_position(&self, position: &Position) -> RiskResult<()>;
    
    async fn delete_position(&self, user_id: Uuid, instrument_id: &str) -> RiskResult<()>;
    
    async fn list_positions(&self, user_id: Uuid) -> RiskResult<Vec<Position>>;
}
