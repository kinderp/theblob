#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{
    NodeId, PhysicalNodeSubstrate, PhysicalTestNodeReadiness, SystemArchitecture,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NixOsProbeSnapshot {
    pub node: NodeId,
    pub observed_architecture: SystemArchitecture,
    pub observed_substrate: PhysicalNodeSubstrate,
    pub on_external_power: Option<bool>,
    pub free_space_bytes: Option<u64>,
    pub current_boot_generation: Option<String>,
    pub rollback_reference: Option<String>,
    pub running_system_store_path: Option<String>,
    pub boot_profile_store_path: Option<String>,
    pub observed_at_unix_ms: u64,
    pub evidence: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSafetyConfirmations {
    pub enrolled: bool,
    pub trusted: bool,
    pub storage_health_ok: bool,
    pub local_console_recovery_confirmed: bool,
    /// Optional physical confirmation when sysfs cannot establish power state.
    pub external_power_override: Option<bool>,
}

impl NixOsProbeSnapshot {
    pub fn to_readiness(&self, confirmations: &NodeSafetyConfirmations) -> PhysicalTestNodeReadiness {
        PhysicalTestNodeReadiness {
            node: self.node.clone(),
            observed_architecture: self.observed_architecture.clone(),
            observed_substrate: self.observed_substrate.clone(),
            enrolled: confirmations.enrolled,
            trusted: confirmations.trusted,
            on_external_power: self
                .on_external_power
                .or(confirmations.external_power_override)
                .unwrap_or(false),
            free_space_bytes: self.free_space_bytes.unwrap_or(0),
            storage_health_ok: confirmations.storage_health_ok,
            current_boot_generation: self.current_boot_generation.clone(),
            rollback_reference: self.rollback_reference.clone(),
            local_console_recovery_confirmed: confirmations.local_console_recovery_confirmed,
            observed_at_unix_ms: self.observed_at_unix_ms,
        }
    }

    pub fn evidence_lines(&self) -> Vec<String> {
        let mut lines = self.evidence.clone();
        lines.push(format!("probe-node:{}", self.node));
        lines.push(format!("probe-architecture:{:?}", self.observed_architecture));
        lines.push(format!("probe-substrate:{:?}", self.observed_substrate));
        lines.push(format!(
            "probe-external-power:{}",
            self.on_external_power
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".into())
        ));
        lines.push(format!(
            "probe-free-space-bytes:{}",
            self.free_space_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".into())
        ));
        lines.push(format!(
            "probe-current-boot-generation:{}",
            self.current_boot_generation.as_deref().unwrap_or("unknown")
        ));
        lines.push(format!(
            "probe-rollback-reference:{}",
            self.rollback_reference.as_deref().unwrap_or("unknown")
        ));
        lines.push(format!("probe-observed-at-unix-ms:{}", self.observed_at_unix_ms));
        lines
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NixOsProbeError {
    UnsupportedArchitecture(String),
    ReadFailed { path: String, message: String },
    NotNixOs { observed_id: Option<String> },
    ClockBeforeUnixEpoch,
}

pub struct NixOsReadOnlyProbe;

impl NixOsReadOnlyProbe {
    pub fn observe_current_host(node: impl Into<NodeId>) -> Result<NixOsProbeSnapshot, NixOsProbeError> {
        let node = node.into();
        let observed_architecture = architecture_from_name(env::consts::ARCH)?;

        let os_release_path = Path::new("/etc/os-release");
        let os_release = read_text(os_release_path)?;
        let observed_id = parse_os_release_id(&os_release);
        if observed_id.as_deref() != Some("nixos") {
            return Err(NixOsProbeError::NotNixOs { observed_id });
        }

        let mut evidence = vec!["os-release:ID=nixos".into()];
        let mut warnings = Vec::new();

        let on_external_power = match probe_external_power(Path::new("/sys/class/power_supply")) {
            Ok(observation) => {
                evidence.extend(observation.evidence);
                observation.on_external_power
            }
            Err(message) => {
                warnings.push(format!("power-state-unavailable:{message}"));
                None
            }
        };

        let free_space_bytes = match probe_free_space_bytes(Path::new("/nix/store")) {
            Ok(bytes) => {
                evidence.push(format!("df:/nix/store:available-bytes={bytes}"));
                Some(bytes)
            }
            Err(message) => {
                warnings.push(format!("free-space-unavailable:{message}"));
                None
            }
        };

        let generation = probe_boot_profile(Path::new("/nix/var/nix/profiles"));
        evidence.extend(generation.evidence);
        warnings.extend(generation.warnings);

        let running_system_store_path = canonical_store_path(Path::new("/run/current-system"));
        let boot_profile_store_path = canonical_store_path(Path::new("/nix/var/nix/profiles/system"));

        if let (Some(running), Some(boot)) = (&running_system_store_path, &boot_profile_store_path) {
            if running != boot {
                warnings.push("running-system-differs-from-boot-profile".into());
            }
        }

        let observed_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| NixOsProbeError::ClockBeforeUnixEpoch)?
            .as_millis()
            .min(u64::MAX as u128) as u64;

        Ok(NixOsProbeSnapshot {
            node,
            observed_architecture,
            observed_substrate: PhysicalNodeSubstrate::NixOs,
            on_external_power,
            free_space_bytes,
            current_boot_generation: generation.current_generation.clone(),
            // For `nixos-rebuild test`, reboot returns to the current boot-default
            // system profile. The rollback reference is therefore that known
            // boot generation, not an arbitrary older generation.
            rollback_reference: generation.current_generation,
            running_system_store_path,
            boot_profile_store_path,
            observed_at_unix_ms,
            evidence,
            warnings,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PowerObservation {
    on_external_power: Option<bool>,
    evidence: Vec<String>,
}

fn architecture_from_name(value: &str) -> Result<SystemArchitecture, NixOsProbeError> {
    match value {
        "x86_64" => Ok(SystemArchitecture::X86_64),
        "aarch64" => Ok(SystemArchitecture::Aarch64),
        other => Err(NixOsProbeError::UnsupportedArchitecture(other.to_owned())),
    }
}

fn read_text(path: &Path) -> Result<String, NixOsProbeError> {
    fs::read_to_string(path).map_err(|error| NixOsProbeError::ReadFailed {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn parse_os_release_id(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if key.trim() != "ID" {
            return None;
        }
        Some(value.trim().trim_matches('"').to_owned())
    })
}

fn probe_external_power(root: &Path) -> Result<PowerObservation, String> {
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    let mut saw_battery = false;
    let mut saw_external_source = false;
    let mut external_online = false;
    let mut evidence = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let supply_type = fs::read_to_string(path.join("type"))
            .unwrap_or_default()
            .trim()
            .to_owned();

        if supply_type == "Battery" {
            saw_battery = true;
            evidence.push(format!("power-supply:{}:type=Battery", entry.file_name().to_string_lossy()));
            continue;
        }

        if matches!(
            supply_type.as_str(),
            "Mains" | "USB" | "USB_C" | "USB_PD" | "USB_DCP" | "USB_CDP" | "USB_ACA"
        ) {
            saw_external_source = true;
            let online = fs::read_to_string(path.join("online"))
                .unwrap_or_default()
                .trim()
                == "1";
            external_online |= online;
            evidence.push(format!(
                "power-supply:{}:type={}:online={}",
                entry.file_name().to_string_lossy(),
                supply_type,
                online
            ));
        }
    }

    let on_external_power = if saw_external_source {
        Some(external_online)
    } else if !saw_battery {
        // A machine with no battery and no AC power_supply device is typically
        // a desktop/server powered externally. This is an OS-level observation,
        // not a substitute for UPS/recovery policy.
        evidence.push("power-supply:no-battery-detected:treat-as-external-power".into());
        Some(true)
    } else {
        // Battery exists but sysfs exposes no source whose `online` state we can
        // verify. Unknown is safer than assuming the charger is connected.
        None
    };

    Ok(PowerObservation {
        on_external_power,
        evidence,
    })
}

fn probe_free_space_bytes(path: &Path) -> Result<u64, String> {
    let output = Command::new("df")
        .arg("-Pk")
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }

    parse_df_available_bytes(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| "could not parse POSIX df output".into())
}

fn parse_df_available_bytes(output: &str) -> Option<u64> {
    let data_line = output.lines().filter(|line| !line.trim().is_empty()).last()?;
    let available_kib = data_line.split_whitespace().nth(3)?.parse::<u64>().ok()?;
    available_kib.checked_mul(1024)
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct BootProfileObservation {
    current_generation: Option<String>,
    evidence: Vec<String>,
    warnings: Vec<String>,
}

fn probe_boot_profile(profile_dir: &Path) -> BootProfileObservation {
    let mut observation = BootProfileObservation::default();
    let system_link = profile_dir.join("system");

    match fs::read_link(&system_link) {
        Ok(target) => {
            observation.evidence.push(format!(
                "nixos-system-profile-link:{}",
                target.display()
            ));
            if let Some(generation) = parse_generation_from_path(&target) {
                observation.current_generation = Some(format!("nixos-generation:{generation}"));
            } else {
                observation
                    .warnings
                    .push("could-not-parse-current-system-generation".into());
            }
        }
        Err(error) => observation
            .warnings
            .push(format!("system-profile-unavailable:{error}")),
    }

    observation
}

fn parse_generation_from_path(path: &Path) -> Option<u64> {
    parse_generation_name(path.file_name()?.to_str()?)
}

fn parse_generation_name(name: &str) -> Option<u64> {
    let number = name.strip_prefix("system-")?.strip_suffix("-link")?;
    number.parse().ok()
}

fn canonical_store_path(path: &Path) -> Option<String> {
    fs::canonicalize(path)
        .ok()
        .and_then(|resolved| resolved.to_str().map(ToOwned::to_owned))
}

#[cfg(test)]
mod tests {
    use std::process;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "theblob-node-probe-{name}-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("temporary directory");
        path
    }

    #[test]
    fn parses_quoted_os_release_id() {
        assert_eq!(
            parse_os_release_id("NAME=\"NixOS\"\nID=\"nixos\"\n"),
            Some("nixos".into())
        );
    }

    #[test]
    fn parses_system_generation_link_name() {
        assert_eq!(parse_generation_name("system-42-link"), Some(42));
        assert_eq!(parse_generation_name("system-link"), None);
    }

    #[test]
    fn parses_posix_df_available_space() {
        let output = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/test 100000 25000 75000 25% /nix/store\n";
        assert_eq!(parse_df_available_bytes(output), Some(75_000 * 1024));
    }

    #[test]
    fn power_probe_detects_online_mains() {
        let root = temp_dir("power-online");
        let ac = root.join("AC");
        let bat = root.join("BAT0");
        fs::create_dir_all(&ac).expect("AC dir");
        fs::create_dir_all(&bat).expect("battery dir");
        fs::write(ac.join("type"), "Mains\n").expect("type");
        fs::write(ac.join("online"), "1\n").expect("online");
        fs::write(bat.join("type"), "Battery\n").expect("battery type");

        let observation = probe_external_power(&root).expect("power observation");
        assert_eq!(observation.on_external_power, Some(true));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn power_probe_keeps_battery_only_state_unknown() {
        let root = temp_dir("power-unknown");
        let bat = root.join("BAT0");
        fs::create_dir_all(&bat).expect("battery dir");
        fs::write(bat.join("type"), "Battery\n").expect("battery type");

        let observation = probe_external_power(&root).expect("power observation");
        assert_eq!(observation.on_external_power, None);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn current_boot_generation_becomes_test_activation_rollback_reference() {
        let snapshot = NixOsProbeSnapshot {
            node: NodeId::from("node:test"),
            observed_architecture: SystemArchitecture::X86_64,
            observed_substrate: PhysicalNodeSubstrate::NixOs,
            on_external_power: Some(true),
            free_space_bytes: Some(16 * 1024 * 1024 * 1024),
            current_boot_generation: Some("nixos-generation:42".into()),
            rollback_reference: Some("nixos-generation:42".into()),
            running_system_store_path: Some("/nix/store/running".into()),
            boot_profile_store_path: Some("/nix/store/boot".into()),
            observed_at_unix_ms: 123,
            evidence: Vec::new(),
            warnings: Vec::new(),
        };

        let readiness = snapshot.to_readiness(&NodeSafetyConfirmations {
            enrolled: true,
            trusted: true,
            storage_health_ok: true,
            local_console_recovery_confirmed: true,
            external_power_override: None,
        });

        assert_eq!(
            readiness.rollback_reference.as_deref(),
            Some("nixos-generation:42")
        );
        assert!(readiness.on_external_power);
    }

    #[test]
    fn unknown_power_never_becomes_safe_without_confirmation() {
        let snapshot = NixOsProbeSnapshot {
            node: NodeId::from("node:test"),
            observed_architecture: SystemArchitecture::X86_64,
            observed_substrate: PhysicalNodeSubstrate::NixOs,
            on_external_power: None,
            free_space_bytes: Some(16 * 1024 * 1024 * 1024),
            current_boot_generation: Some("nixos-generation:42".into()),
            rollback_reference: Some("nixos-generation:42".into()),
            running_system_store_path: None,
            boot_profile_store_path: None,
            observed_at_unix_ms: 123,
            evidence: Vec::new(),
            warnings: Vec::new(),
        };

        let unconfirmed = snapshot.to_readiness(&NodeSafetyConfirmations {
            enrolled: true,
            trusted: true,
            storage_health_ok: true,
            local_console_recovery_confirmed: true,
            external_power_override: None,
        });
        assert!(!unconfirmed.on_external_power);

        let confirmed = snapshot.to_readiness(&NodeSafetyConfirmations {
            enrolled: true,
            trusted: true,
            storage_health_ok: true,
            local_console_recovery_confirmed: true,
            external_power_override: Some(true),
        });
        assert!(confirmed.on_external_power);
    }
}
