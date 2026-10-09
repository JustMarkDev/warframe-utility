use serde::{Serialize, Deserialize};
use std::fmt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyndicateState {
    pub faction_key: String,
    pub standing: i32,
    pub rank: i32,
}

impl SyndicateState {
    pub fn new(faction_key: &str, standing: i32, rank: i32) -> Self {
        Self {
            faction_key: faction_key.to_string(),
            standing,
            rank,
        }
    }

    pub fn max_standing(&self) -> i32 {
        match self.rank {
            5 => 132000,
            4 => 99000,
            3 => 70000,
            2 => 44000,
            1 => 22000,
            _ => 5000,
        }
    }

    pub fn listable_quantity(&self) -> i32 {
        let cost = get_syndicate_cost(&self.faction_key);
        self.standing / cost
    }
}

pub fn get_syndicate_cost(_faction_key: &str) -> i32 {
    25000
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum AppError {
    Overlap {
        item_slug: String,
        eligible_factions: Vec<String>,
    },
    InsufficientStanding {
        item_slug: String,
    },
    AuthExpired {
        message: String,
    },
    Io(String),
    Network(String),
    Other(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Overlap { item_slug, eligible_factions } => {
                write!(f, "Item '{}' belongs to multiple eligible factions: {:?}", item_slug, eligible_factions)
            }
            AppError::InsufficientStanding { item_slug } => {
                write!(f, "No represented faction has enough standing to sell '{}'", item_slug)
            }
            AppError::AuthExpired { message } => write!(f, "{}", message),
            AppError::Io(err) => write!(f, "IO Error: {}", err),
            AppError::Network(err) => write!(f, "Network/API Error: {}", err),
            AppError::Other(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err.to_string())
    }
}

pub fn get_syndicate_mods(faction_key: &str, current_rank: i32) -> Vec<&'static str> {
    let mut mods = Vec::new();
    for r in 0..=current_rank {
        mods.extend(get_syndicate_mods_at_rank(faction_key, r));
    }
    mods
}

pub fn get_syndicate_mods_at_rank(faction: &str, rank: i32) -> Vec<&'static str> {
    match (faction, rank) {
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syndicate_state_limits() {
        let state = SyndicateState::new("steel_meridian", 100000, 5);
        assert_eq!(state.standing, 100000);
        assert_eq!(state.rank, 5);
        assert_eq!(state.max_standing(), 132000);
        assert_eq!(state.listable_quantity(), 4);
    }

    #[test]
    fn test_standing_bounds_clamping_ranges() {
        let mut state = SyndicateState::new("steel_meridian", 150000, 5);
        let max_val = state.max_standing();
        state.standing = i32::min(150000, max_val);
        assert_eq!(state.standing, 132000);

        state.rank = 4;
        let max_val = state.max_standing();
        state.standing = i32::min(state.standing, max_val);
        assert_eq!(state.standing, 99000);
    }

    #[test]
    fn test_listable_quantity_calculations() {
        let mut state = SyndicateState::new("steel_meridian", 50000, 5);
        assert_eq!(state.listable_quantity(), 2);

        state.standing = 24999;
        assert_eq!(state.listable_quantity(), 0);

        state.standing = 0;
        assert_eq!(state.listable_quantity(), 0);
    }

    #[test]
    fn test_get_syndicate_mods() {
        let mods_rank5 = get_syndicate_mods("steel_meridian", 5);
        assert!(mods_rank5.contains(&"scattered_justice"));
        assert!(mods_rank5.contains(&"path_of_statues"));

        let mods_rank3 = get_syndicate_mods("steel_meridian", 3);
        assert_eq!(mods_rank3.len(), 0);
    }
}
