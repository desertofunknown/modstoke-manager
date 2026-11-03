use super::launch;
use super::native_config::{self, NativeConfig};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

type Choices = Option<&'static [&'static str]>;

const GAME_MODES: &[&str] = &["Normal", "Hard", "HardSurvival", "Peaceful", "Creative", "Custom"];
const LOW_NORMAL_HIGH: Choices = Some(&["Low", "Normal", "High"]);
const LOW_DEFAULT_HIGH: Choices = Some(&["Low", "Default", "High"]);
const LENGTHS: Choices = Some(&["Short", "Default", "Long", "Realistic"]);
const SEASONS: Choices = Some(&["Spring", "Summer", "Autumn", "Winter"]);
const SURVIVAL: Choices = Some(&["Off", "Normal", "Hard"]);
const PVP: Choices = Some(&["Off", "Reduced", "Normal"]);

// The only keys the dedicated server reads from these objects; it ignores the rest. Keys are
// case-sensitive there. Choices are matched without case, but Manager writes these names.
// `None` marks a JSON boolean.
const GAME_SETTINGS: &[(&str, Choices)] = &[
    ("Gameplay.TreeRegrowth", None),
    ("Structure.Damage", None),
];
const CUSTOM_SETTINGS: &[(&str, Choices)] = &[
    ("GameSetting.Vail.EnemySpawn", None),
    ("GameSetting.Vail.EnemyHealth", LOW_NORMAL_HIGH),
    ("GameSetting.Vail.EnemyDamage", LOW_NORMAL_HIGH),
    ("GameSetting.Vail.EnemyArmour", LOW_NORMAL_HIGH),
    ("GameSetting.Vail.EnemyAggression", LOW_NORMAL_HIGH),
    ("GameSetting.Vail.EnemySearchParties", LOW_NORMAL_HIGH),
    ("GameSetting.Vail.AnimalSpawnRate", LOW_NORMAL_HIGH),
    ("GameSetting.Environment.StartingSeason", SEASONS),
    ("GameSetting.Environment.SeasonLength", LENGTHS),
    ("GameSetting.Environment.DayLength", LENGTHS),
    ("GameSetting.Environment.PrecipitationFrequency", LOW_DEFAULT_HIGH),
    ("GameSetting.Environment.InstantSleepSeasonSwitch", None),
    ("GameSetting.Survival.ConsumableEffects", SURVIVAL),
    ("GameSetting.Survival.PlayerStatsDamage", SURVIVAL),
    ("GameSetting.Survival.ColdPenalties", SURVIVAL),
    ("GameSetting.Survival.StatRegenerationPenalty", SURVIVAL),
    ("GameSetting.Survival.ReducedFoodInContainers", None),
    ("GameSetting.Survival.ReducedAmmoInContainers", None),
    ("GameSetting.Survival.SingleUseContainers", None),
    ("GameSetting.Survival.BuildingResistance", LOW_NORMAL_HIGH),
    ("GameSetting.Survival.CreativeMode", None),
    ("GameSetting.Survival.PlayersImmortalMode", None),
    ("GameSetting.Survival.OneHitToCutTrees", None),
    ("GameSetting.Construction.NoCuttingsSpawn", None),
    ("GameSetting.FreeForm.ForcePlaceFullLoad", None),
    ("GameSetting.Multiplayer.Cheats", None),
    ("GameSetting.Multiplayer.PvpDamage", PVP),
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    key: &'static str,
    choices: Choices,
    value: Option<Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSettingsState {
    config_revision: String,
    game_mode: Option<Value>,
    save_interval: Option<Value>,
    idle_day_cycle_speed: Option<Value>,
    idle_target_framerate: Option<Value>,
    active_target_framerate: Option<Value>,
    lan_only: Option<Value>,
    skip_network_accessibility_test: Option<Value>,
    game_settings: Vec<Setting>,
    custom_settings: Vec<Setting>,
    ignored_keys: Vec<String>,
}

/// Fields left out stay as they are. In the two objects, null removes a key so the game default applies.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameSettingsChange {
    game_mode: Option<String>,
    save_interval: Option<u32>,
    idle_day_cycle_speed: Option<f64>,
    idle_target_framerate: Option<u32>,
    active_target_framerate: Option<u32>,
    lan_only: Option<bool>,
    skip_network_accessibility_test: Option<bool>,
    #[serde(default)]
    game_settings: BTreeMap<String, Option<Value>>,
    #[serde(default)]
    custom_settings: BTreeMap<String, Option<Value>>,
}

// Keeps entry order, so a rewritten object differs from the original only where it changed.
struct OrderedObject(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for OrderedObject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntriesVisitor;

        impl<'de> Visitor<'de> for EntriesVisitor {
            type Value = OrderedObject;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object with unique keys")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut entries: Vec<(String, Value)> = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    if entries.iter().any(|(existing, _)| *existing == key) {
                        return Err(de::Error::custom("duplicate key"));
                    }
                    entries.push((key, map.next_value()?));
                }
                Ok(OrderedObject(entries))
            }
        }

        deserializer.deserialize_map(EntriesVisitor)
    }
}

fn entries(config: &NativeConfig, name: &str) -> Result<Vec<(String, Value)>, String> {
    let Some(raw) = config.raw_property(name)? else { return Ok(Vec::new()); };
    // The server treats null like an empty object.
    let object: Option<OrderedObject> = serde_json::from_str(raw)
        .map_err(|_| format!("dedicatedserver.cfg {name} must be a JSON object without duplicate keys."))?;
    Ok(object.map_or_else(Vec::new, |object| object.0))
}

fn property(config: &NativeConfig, name: &str) -> Result<Option<Value>, String> {
    config.raw_property(name)?.map(|raw| serde_json::from_str(raw)
        .map_err(|_| "Could not read the native configuration value.".to_string())).transpose()
}

fn state(config: &NativeConfig) -> Result<GameSettingsState, String> {
    let mut ignored_keys = Vec::new();
    let mut settings = |name: &str, schema: &[(&'static str, Choices)]| -> Result<Vec<Setting>, String> {
        let entries = entries(config, name)?;
        ignored_keys.extend(entries.iter().filter(|(key, _)| !schema.iter().any(|(known, _)| *known == key.as_str()))
            .map(|(key, _)| format!("{name}: {key}")));
        Ok(schema.iter().map(|&(key, choices)| Setting {
            key, choices,
            value: entries.iter().find(|(existing, _)| existing.as_str() == key).map(|(_, value)| value.clone()),
        }).collect())
    };
    let game_settings = settings("GameSettings", GAME_SETTINGS)?;
    let custom_settings = settings("CustomGameModeSettings", CUSTOM_SETTINGS)?;
    Ok(GameSettingsState {
        config_revision: config.revision.clone(),
        game_mode: property(config, "GameMode")?,
        save_interval: property(config, "SaveInterval")?,
        idle_day_cycle_speed: property(config, "IdleDayCycleSpeed")?,
        idle_target_framerate: property(config, "IdleTargetFramerate")?,
        active_target_framerate: property(config, "ActiveTargetFramerate")?,
        lan_only: property(config, "LanOnly")?,
        skip_network_accessibility_test: property(config, "SkipNetworkAccessibilityTest")?,
        game_settings, custom_settings, ignored_keys,
    })
}

fn object_text(config: &NativeConfig, name: &str, schema: &[(&str, Choices)],
    changes: &BTreeMap<String, Option<Value>>) -> Result<String, String> {
    let mut entries = entries(config, name)?;
    for (key, value) in changes {
        let choices = schema.iter().find(|(known, _)| *known == key.as_str()).map(|(_, choices)| *choices)
            .ok_or_else(|| format!("{key} is not a setting the dedicated server reads."))?;
        let valid = match (value, choices) {
            (None, _) | (Some(Value::Bool(_)), None) => true,
            (Some(Value::String(text)), Some(choices)) => choices.contains(&text.as_str()),
            _ => false,
        };
        if !valid { return Err(format!("Choose one of the listed values for {key}.")); }
        let position = entries.iter().position(|(existing, _)| existing == key);
        match (position, value) {
            (Some(index), Some(value)) => entries[index].1 = value.clone(),
            (Some(index), None) => { entries.remove(index); }
            (None, Some(value)) => entries.push((key.clone(), value.clone())),
            (None, None) => {}
        }
    }
    if entries.is_empty() { return Ok("{}".into()); }
    let items = entries.iter().map(|(key, value)| format!("{}: {value}", Value::String(key.clone())));
    // Lay entries out like the rest of the file: one per line, a level deeper than the property.
    Ok(match config.property_indent(name)? {
        Some(indent) => {
            let newline = config.newline();
            let inner = format!("{newline}{indent}{}", if indent.is_empty() { "  " } else { indent });
            format!("{{{inner}{}{newline}{indent}}}", items.collect::<Vec<_>>().join(&format!(",{inner}")))
        }
        None => format!("{{{}}}", items.collect::<Vec<_>>().join(", ")),
    })
}

pub(crate) fn apply(config: &NativeConfig, change: &GameSettingsChange) -> Result<Option<NativeConfig>, String> {
    let mut properties: Vec<(&str, String)> = Vec::new();
    if let Some(mode) = &change.game_mode {
        if !GAME_MODES.contains(&mode.as_str()) { return Err("Choose one of the listed game modes.".into()); }
        properties.push(("GameMode", Value::String(mode.clone()).to_string()));
    }
    if let Some(seconds) = change.save_interval {
        if !(60..=86_400).contains(&seconds) { return Err("Choose an autosave interval from 1 minute to 24 hours.".into()); }
        properties.push(("SaveInterval", seconds.to_string()));
    }
    if let Some(speed) = change.idle_day_cycle_speed {
        if !(0.0..=1.0).contains(&speed) { return Err("Idle day cycle speed must be from 0 to 1.".into()); }
        properties.push(("IdleDayCycleSpeed", serde_json::to_string(&speed).map_err(|error| error.to_string())?));
    }
    if let Some(rate) = change.idle_target_framerate {
        if !(1..=240).contains(&rate) { return Err("Idle frame rate must be from 1 to 240.".into()); }
        properties.push(("IdleTargetFramerate", rate.to_string()));
    }
    if let Some(rate) = change.active_target_framerate {
        if !(10..=240).contains(&rate) { return Err("Active frame rate must be from 10 to 240.".into()); }
        properties.push(("ActiveTargetFramerate", rate.to_string()));
    }
    if let Some(enabled) = change.lan_only { properties.push(("LanOnly", enabled.to_string())); }
    if let Some(skip) = change.skip_network_accessibility_test { properties.push(("SkipNetworkAccessibilityTest", skip.to_string())); }
    if !change.game_settings.is_empty() {
        properties.push(("GameSettings", object_text(config, "GameSettings", GAME_SETTINGS, &change.game_settings)?));
    }
    // The server refuses a Custom game whose CustomGameModeSettings is null.
    let custom_null = config.raw_property("CustomGameModeSettings")? == Some("null");
    if !change.custom_settings.is_empty() || (custom_null && change.game_mode.as_deref() == Some("Custom")) {
        properties.push(("CustomGameModeSettings", object_text(config, "CustomGameModeSettings", CUSTOM_SETTINGS, &change.custom_settings)?));
    }

    let mut changed: Option<NativeConfig> = None;
    for (name, value) in properties {
        changed = Some(changed.as_ref().unwrap_or(config).with_property(name, &value)?);
    }
    Ok(changed.filter(|changed| changed.revision != config.revision))
}

#[tauri::command]
pub async fn read_server_game_settings(app: tauri::AppHandle, executable: String, installation_id: String) -> Result<GameSettingsState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (_, data) = launch::server_folders(&app, &executable, &installation_id)?;
        let data = data.ok_or("Choose the server data folder before editing game settings.")?;
        state(&native_config::read(Path::new(&data))?)
    }).await.map_err(|error| error.to_string())?
}
