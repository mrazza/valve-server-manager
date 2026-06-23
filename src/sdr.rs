use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Relay {
    pub ipv4: String,
    pub port_range: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PoP {
    pub desc: String,
    pub aliases: Option<Vec<String>>,
    pub relays: Option<Vec<Relay>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SdrConfig {
    pub pops: HashMap<String, PoP>,
    pub success: bool,
}

/// Parses the SDR configuration from a JSON string.
pub fn parse_sdr_config(json: &str) -> Result<HashMap<String, PoP>, serde_json::Error> {
    let config = serde_json::from_str::<SdrConfig>(json)?;
    Ok(config.pops.into_iter()
        .filter(|(_, pop)| pop.relays.is_some() && !pop.relays.as_ref().unwrap().is_empty())
        .collect())
}

/// Fetches the latest SDR configuration from Valve's API.
/// If the request fails, it falls back to the embedded JSON.
pub fn fetch_sdr_config() -> HashMap<String, PoP> {
    let url = "https://api.steampowered.com/ISteamApps/GetSDRConfig/v1/?appid=730";
    
    match ureq::get(url).call() {
        Ok(response) => {
            if let Ok(body) = response.into_string() {
                if let Ok(pops) = parse_sdr_config(&body) {
                    return pops;
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to fetch online SDR config: {}. Using fallback.", e);
        }
    }

    // Fallback to embedded config
    get_fallback_sdr_config()
}

fn get_fallback_sdr_config() -> HashMap<String, PoP> {
    let fallback_data = include_str!("fallback_sdr.json");
    parse_sdr_config(fallback_data).unwrap_or_default()
}

/// Maps PoP codes to their respective geographical regions for cleaner UI categorisation.
pub fn get_region_for_pop(code: &str) -> &'static str {
    match code.to_lowercase().as_str() {
        "sea" | "ord" | "lax" | "dfw" | "atl" | "iad" | "pdx" | "phx" | "mco" | "den" | "eat" | "ord2" | "iad2" | "sea2" | "lax2" => "North America",
        "ams" | "fra" | "lhr" | "cdg" | "mad" | "sto" | "vie" | "waw" | "par" | "muc" | "hel" | "gva" | "prg" | "bud" | "fsn" | "ams2" | "fra2" | "sto2" => "Europe",
        "hkg" | "sgp" | "tyo" | "sel" | "bom" | "dxb" | "shb" | "shh" | "can" | "wuh" | "tsn" | "pek" | "bom2" | "maa2" | "seo" | "sgp2" | "tyo2" | "sel2" | "dxb2" | "hkg2" |
        "pekm" | "pekt" | "peku" | "ctum" | "ctut" | "ctuu" | "pvgm" | "pvgt" | "pvgu" | "tgdm" | "tgdt" | "tgdu" | "pwg" | "pww" | "pvg" => "Asia",
        "gru" | "scl" | "lim" | "eze" | "bue" | "gru2" | "scl2" | "lim2" => "South America",
        "syd" | "akl" | "syd2" | "akl2" => "Oceania",
        "jnb" | "cpt" | "jnb2" | "cpt2" => "Africa",
        _ => "Other / Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_sdr_config_parsing() {
        let pops = get_fallback_sdr_config();
        assert!(!pops.is_empty(), "Fallback PoPs map should not be empty");
        
        assert!(pops.contains_key("sea"), "Should contain Seattle (sea) relay");
        assert!(pops.contains_key("fra"), "Should contain Frankfurt (fra) relay");

        let sea_pop = pops.get("sea").unwrap();
        assert!(sea_pop.desc.contains("Seattle"));
        
        assert!(sea_pop.relays.is_some());
        assert!(!sea_pop.relays.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_region_mapping() {
        assert_eq!(get_region_for_pop("sea"), "North America");
        assert_eq!(get_region_for_pop("fra"), "Europe");
        assert_eq!(get_region_for_pop("hkg"), "Asia");
        assert_eq!(get_region_for_pop("syd"), "Oceania");
        assert_eq!(get_region_for_pop("gru"), "South America");
        assert_eq!(get_region_for_pop("jnb"), "Africa");
        assert_eq!(get_region_for_pop("xyz"), "Other / Unknown");
    }

    #[test]
    fn test_parse_sdr_config_invalid() {
        let res = parse_sdr_config("{ invalid json }");
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_sdr_config_empty_relays() {
        let sample = r#"{
            "success": true,
            "pops": {
                "ctu": {
                    "desc": "Chengdu",
                    "relays": []
                },
                "sea": {
                    "desc": "Seattle",
                    "relays": [
                        {
                            "ipv4": "192.69.96.0/22"
                        }
                    ]
                }
            }
        }"#;
        let pops = parse_sdr_config(sample).unwrap();
        assert!(!pops.contains_key("ctu"), "ctu should be filtered out because relays are empty");
        assert!(pops.contains_key("sea"), "sea should be kept because it has relays");
    }
}
