import fs from "fs";
import path from "path";

const API_URL = "https://raw.githubusercontent.com/WFCD/warframe-items/master/data/json/Mods.json";

// Mapping of Syndicate names from API to our internal keys
const SYNDICATES = {
  "steel meridian": "steel_meridian",
  "arbiters of hexis": "arbiters_of_hexis",
  "cephalon suda": "cephalon_suda",
  "perrin sequence": "perrin_sequence",
  "the perrin sequence": "perrin_sequence",
  "red veil": "red_veil",
  "new loka": "new_loka",
};

// Mapping of Rank titles to numerical ranks (4 or 5)
const RANKS = {
  // Rank 4
  protector: 4,
  crusade: 4,
  wise: 4,
  opportunity: 4,
  venerated: 4,
  defender: 4,
  // Rank 5
  general: 5,
  maxim: 5,
  genius: 5,
  partner: 5,
  exalted: 5,
  tycoon: 5,
};

async function main() {
  console.log(`Fetching live mods database from ${API_URL}...`);
  try {
    const res = await fetch(API_URL);
    if (!res.ok) {
      throw new Error(`HTTP error! Status: ${res.status}`);
    }
    const mods = await res.json();
    console.log(`Successfully fetched ${mods.length} mods! Processing...`);

    const syndicateMap = {
      steel_meridian: { 4: new Set(), 5: new Set() },
      arbiters_of_hexis: { 4: new Set(), 5: new Set() },
      cephalon_suda: { 4: new Set(), 5: new Set() },
      perrin_sequence: { 4: new Set(), 5: new Set() },
      red_veil: { 4: new Set(), 5: new Set() },
      new_loka: { 4: new Set(), 5: new Set() },
    };

    const allSlugsSet = new Set();

    for (const item of mods) {
      if (!item.drops || !item.name) continue;

      for (const drop of item.drops) {
        if (!drop.location) continue;

        // Split "Steel Meridian, Protector" into ["Steel Meridian", "Protector"]
        const parts = drop.location.split(",");
        if (parts.length < 2) continue;

        const syndicateRaw = parts[0].trim().toLowerCase();
        const rankRaw = parts[1].trim().toLowerCase();

        const factionKey = SYNDICATES[syndicateRaw];
        const rankNum = RANKS[rankRaw];

        if (factionKey && rankNum) {
          // Normalize slug to match warframe.market item slugs:
          // Lowercase, spaces replaced by underscores, remove single quotes, dashes to underscores.
          let slug = item.name.toLowerCase().replace(/[']/g, "").replace(/[\s-]/g, "_");

          syndicateMap[factionKey][rankNum].add(slug);
          allSlugsSet.add(slug);
        }
      }
    }

    const allSlugs = Array.from(allSlugsSet).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    console.log(`Found ${allSlugs.length} unique Syndicate augment/weapon mods!`);

    // 1. Generate domain.rs contents
    await generateRustDomain(syndicateMap);

    console.log("Successfully completed mod list synchronization!");
  } catch (error) {
    console.error("Failed to sync mods:", error);
    process.exit(1);
  }
}

async function generateRustDomain(syndicateMap) {
  const domainPath = path.resolve("src-tauri", "src", "domain.rs");
  console.log(`Writing regenerated ${domainPath}...`);

  let code = `use serde::{Serialize, Deserialize};
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
`;

  // Sort factions for deterministic outputs
  const factionsList = Object.keys(syndicateMap).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  for (const faction of factionsList) {
    for (const rank of [4, 5]) {
      const items = Array.from(syndicateMap[faction][rank]).sort((a, b) =>
        a < b ? -1 : a > b ? 1 : 0,
      );
      if (items.length === 0) continue;

      code += `        ("${faction}", ${rank}) => vec![
`;
      // Chunk items to keep it pretty
      let line = "            ";
      for (let i = 0; i < items.length; i++) {
        const itemStr = `"${items[i]}", `;
        if ((line + itemStr).length > 100) {
          code += line.trimEnd() + "\n";
          line = "            " + itemStr;
        } else {
          line += itemStr;
        }
      }
      if (line.trim() !== "") {
        code += line.trimEnd() + "\n";
      }
      code += `        ],\n`;
    }
  }

  code += `        _ => vec![],
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
`;

  await fs.promises.writeFile(domainPath, code, "utf-8");
}

void main();
