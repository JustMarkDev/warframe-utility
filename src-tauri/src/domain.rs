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
        ("arbiters_of_hexis", 5) => vec![
            "assimilate", "axios_javelineers", "calm_&_frenzy", "capacitance",
            "cataclysmic_continuum", "cathode_current", "celestial_stomp", "chaos_sphere",
            "chromatic_blade", "coil_recharge", "conductive_sphere", "damage_decoy",
            "desiccations_curse", "duality", "elemental_sandstorm", "elusive_retribution",
            "endless_lullaby", "energy_transfer", "enveloping_cloud", "explosive_legerdemain",
            "furious_javelin", "hall_of_malevolence", "hushed_invisibility", "intrepid_stand",
            "irradiating_disarm", "jades_judgment", "kumihimo_loading", "lasting_covenant",
            "mach_crash", "mending_splinters", "mind_freak", "negation_armor", "noctua_swarm",
            "omikujis_fortune", "pacifying_bolts", "peaceful_provocation", "primal_rage",
            "radiant_finish", "reactive_storm", "repair_dispensary", "reverse_rotorswell",
            "rift_haven", "rift_torrent", "rising_storm", "safeguard", "safeguard_switch",
            "savior_decoy", "seeker", "seeking_shuriken", "shattered_storm", "shock_trooper",
            "shocking_speed", "smoke_shadow", "spectrosiphon", "surging_dash", "teleport_rush",
            "temporal_artillery", "temporal_erosion", "tharros_lethality", "thermal_transfer",
            "total_eclipse", "transistor_shield", "tribunal", "warding_thurible", "warriors_rest",
        ],
        ("cephalon_suda", 4) => vec![
            "entropy_burst", "entropy_detonation", "entropy_flight", "entropy_spike",
        ],
        ("cephalon_suda", 5) => vec![
            "aegis_gale", "afterburn", "antimatter_absorb", "balefire_surge", "biting_frost",
            "blazing_pillage", "blinding_reave", "cataclysmic_continuum", "cataclysmic_gate",
            "chilling_globe", "concentrated_arrow", "conductor", "controlled_slide",
            "critical_surge", "dark_propagation", "divine_retribution", "empowered_quiver",
            "escape_velocity", "everlasting_ward", "explosive_legerdemain", "freeze_force",
            "fused_crucible", "fused_reservoir", "guardian", "guardian_armor", "guided_effigy",
            "hall_of_malevolence", "ice_wave_impedance", "icy_avalanche", "infiltrate",
            "loyal_merulina", "merulina_guardian", "mesmer_shield", "molecular_fission",
            "neutron_star", "noctua_swarm", "partitioned_mallet", "photon_repeater",
            "piercing_navigator", "pilfering_swarm", "pyroclastic_flow", "razor_mortar",
            "reaping_chakram", "resonance", "rift_haven", "rift_torrent", "rousing_plunder",
            "safeguard", "savage_silence", "shadow_haze", "surging_blades", "tesla_bank",
            "the_relentless_lost", "thrall_pact", "tidal_impunity", "total_eclipse", "untime_rift",
            "vampiric_grasp", "vexing_retaliation", "viral_tempest", "wrecking_wall",
        ],
        ("new_loka", 5) => vec![
            "spellbound_harvest",
        ],
        ("perrin_sequence", 5) => vec![
            "abating_link", "abundant_mutation", "aegis_gale", "afterburn", "balefire_surge",
            "blazing_pillage", "blinding_reave", "cathode_current", "champions_blessing",
            "coil_recharge", "concentrated_arrow", "conductive_sphere", "counter_pulse",
            "creeping_terrify", "dark_propagation", "desiccations_curse", "despoil",
            "elemental_sandstorm", "empowered_quiver", "enraged", "eternal_war",
            "everlasting_ward", "fracturing_crush", "greedy_pull", "guardian", "guardian_armor",
            "guided_effigy", "hysterical_assault", "infiltrate", "insatiable", "iron_shrapnel",
            "ironclad_charge", "larva_burst", "mach_crash", "magnetized_discharge",
            "mesmer_shield", "negation_armor", "parasitic_vitality", "photon_repeater",
            "piercing_navigator", "piercing_roar", "pool_of_life", "prolonged_paralysis",
            "razor_mortar", "reinforcing_stomp", "repair_dispensary", "reroot_rampage",
            "resonance", "reverse_rotorswell", "savage_silence", "shadow_haze",
            "shield_of_shadows", "soul_survivor", "spectral_spirit", "swing_line",
            "teeming_virulence", "temporal_artillery", "temporal_erosion", "tesla_bank",
            "thermal_transfer", "thrall_pact", "vampire_leech", "vexing_retaliation",
        ],
        ("red_veil", 5) => vec![
            "accumulating_whipclaw", "airburst_rounds", "anchored_glide", "ballistic_bullseye",
            "beguiling_lantern", "blending_talons", "blood_forge", "capacitance", "catapult",
            "contagion_cloud", "creeping_terrify", "damage_decoy", "despoil", "dread_ward",
            "exothermic", "fireball_frenzy", "funnel_clouds", "gastro", "gourmand",
            "healing_flame", "hearty_nourishment", "hushed_invisibility", "immolated_radiance",
            "ironclad_flight", "irradiating_disarm", "jades_judgment", "jet_stream",
            "lasting_covenant", "lingering_transmutation", "mesas_waltz", "muzzle_flash",
            "ore_gaze", "path_of_statues", "pilfering_strangledome", "prey_of_dynar",
            "prismatic_companion", "razorwing_blitz", "recrystalize", "regenerative_molt",
            "reroot_rampage", "revealing_spores", "rising_storm", "rubble_heap", "safeguard",
            "safeguard_switch", "savior_decoy", "seeking_shuriken", "shield_of_shadows",
            "shock_trooper", "shocking_speed", "smoke_shadow", "soul_survivor", "spectral_spirit",
            "spellbound_harvest", "staggering_shield", "swift_bite", "target_fixation",
            "tectonic_fracture", "teleport_rush", "titanic_rumbler", "transistor_shield",
            "tribunal", "ulfruns_endurance", "valence_formation", "venari_bodyguard", "venom_dose",
            "warding_thurible", "warriors_rest",
        ],
        ("steel_meridian", 4) => vec![
            "justice_blades", "neutralizing_justice", "scattered_justice", "shattering_justice",
        ],
        ("steel_meridian", 5) => vec![
            "abundant_mutation", "accumulating_whipclaw", "antimatter_absorb",
            "ballistic_bullseye", "biting_frost", "blending_talons", "blood_forge", "catapult",
            "chilling_globe", "chromatic_blade", "contagion_cloud", "controlled_slide",
            "divine_retribution", "dread_ward", "escape_velocity", "exothermic", "fireball_frenzy",
            "freeze_force", "furious_javelin", "fused_crucible", "gastro", "gourmand",
            "hallowed_eruption", "hallowed_reckoning", "healing_flame", "hearty_nourishment",
            "ice_wave_impedance", "icy_avalanche", "immolated_radiance", "insatiable",
            "iron_shrapnel", "ironclad_charge", "larva_burst", "mesas_waltz", "molecular_fission",
            "muzzle_flash", "neutron_star", "ore_gaze", "parasitic_vitality", "path_of_statues",
            "phoenix_renewal", "piercing_roar", "pilfering_strangledome", "prey_of_dynar",
            "prismatic_companion", "pyroclastic_flow", "radiant_finish", "reaping_chakram",
            "recrystalize", "regenerative_molt", "reinforcing_stomp", "revealing_spores",
            "rhythm_guard", "rubble_heap", "safeguard", "smite_infusion", "staggering_shield",
            "surging_dash", "tectonic_fracture", "teeming_virulence", "the_relentless_lost",
            "titanic_rumbler", "ulfruns_endurance", "untime_rift", "vampiric_grasp",
            "venari_bodyguard", "venom_dose", "volatile_recompense", "wrath_of_ukko",
            "wrecking_wall",
        ],
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
