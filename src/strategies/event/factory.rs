use crate::entity::sea_orm_active_enums::EventType;
use crate::strategies::event::implementations::{JeopardyPractice, JeopardyTeamStrategy};

use super::implementations::JeopardySingleStrategy;
use super::trait_def::EventStrategy;

pub struct EventStrategyFactory;

impl EventStrategyFactory {
    pub fn create(event_type: &EventType) -> Box<dyn EventStrategy> {
        match event_type {
            EventType::JeopardyPractice => Box::new(JeopardyPractice),
            EventType::JeopardySingle => Box::new(JeopardySingleStrategy),
            EventType::JeopardyTeam => Box::new(JeopardyTeamStrategy),
            _ => todo!(),
        }
    }

    pub fn list_all() -> Vec<(&'static str, EventType)> {
        vec![
            ("练习 Jeopardy", EventType::JeopardyPractice),
            ("单人 Jeopardy", EventType::JeopardySingle),
            ("团队 Jeopardy", EventType::JeopardyTeam),
            ("AWD 攻防", EventType::AwdTeam),
        ]
    }
}
