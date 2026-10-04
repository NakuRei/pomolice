use pomolice_core::{settings::UserSettings, timer_core::TimerCheckpoint};
use serde::{Deserialize, Serialize};

/// Settings file schema version currently supported by this binary.
pub(crate) const SETTINGS_SCHEMA_VERSION: u32 = 1;

/// Settings stored in `settings.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedSettings {
    pub(crate) schema_version: u32,
    pub(crate) settings: UserSettings,
}

/// Whether the settings file could be used at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SettingsFileStatus {
    /// The file was loaded, or none exists yet and defaults apply.
    Ok,
    /// A file exists but could not be used, so defaults are in effect until
    /// the user saves settings again.
    Unusable,
}

pub(crate) const PERSISTED_STATE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedState {
    pub schema_version: u32,
    #[serde(flatten)]
    pub timer: TimerCheckpoint,
}

impl From<TimerCheckpoint> for PersistedState {
    fn from(state: TimerCheckpoint) -> Self {
        Self {
            schema_version: PERSISTED_STATE_SCHEMA_VERSION,
            timer: state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pomolice_core::settings::input::{DaySlotInput, SettingsInput};

    #[test]
    fn settings_round_trip_through_the_file_format() {
        let mut input = SettingsInput::from(UserSettings::default());
        input.weekly_auto_run_config.fri = DaySlotInput {
            enabled: true,
            start_time: "22:00".into(),
            end_time: "02:00".into(),
        };
        let persisted = PersistedSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            settings: UserSettings::try_from(input).expect("valid overnight schedule"),
        };
        let json = serde_json::to_value(&persisted).expect("settings should serialize");
        let restored: PersistedSettings =
            serde_json::from_value(json.clone()).expect("settings should deserialize");
        assert_eq!(restored, persisted);
        assert_eq!(
            json["settings"]["weekly_auto_run_config"]["fri"]["start_time"],
            "22:00"
        );
    }
}
