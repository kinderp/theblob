#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{symlink, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use blob_core::{
    CausalKind, CausalRecord, CausalRecordId, ExperienceProfileId, FeatureState, KernelPolicy,
    SemanticBuildProfile, SystemArchitecture, SystemCandidateId, SystemChannel,
    SystemConstructionMode, SystemFeatureId, SystemFeatureSelection, SystemPriority,
    SystemProfileId, SystemSpec, SystemSpecId, SystemSpecViolation,
};
use blob_nix_nixos::{NixBackendError, NixOsBackend, NixTranslation};
use blob_nix_nixos_materialization_begin::{
    canonical_trusted_candidate, TrustedMaterializationCandidate,
};

pub const DEFAULT_TRUSTED_CANDIDATE_ROOT: &str =
    "/var/lib/theblob/materialization-candidates";
pub const DEFAULT_CANDIDATE_RECEIPT_ROOT: &str =
    "/var/lib/theblob/candidate-manifest-receipts";
pub const DEFAULT_CANDIDATE_STAGING_ROOT: &str =
    "/var/lib/theblob/candidate-source-staging";
pub const DEFAULT_CANDIDATE_SOURCE_GCROOT_ROOT: &str =
    "/nix/var/nix/gcroots/theblob-candidate-sources";
pub const MAX_CANONICAL_SYSTEM_SPEC_BYTES: u64 = 64 * 1024;

const PRODUCER_ACTOR: &str = "blob-nix-nixos-candidate-producer";
const CANDIDATE_SOURCE_NAME: &str = "theblob-candidate-source";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalSystemSpecError {
    TooLarge,
    Malformed,
    NonCanonical,
    InvalidId,
    InvalidSystemSpec(Vec<SystemSpecViolation>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateManifestReceipt {
    pub causal_id: CausalRecordId,
    pub occurred_at_unix_ms: u64,
    pub requester_system_bus_name: String,
    pub manifest_id: String,
    pub candidate: SystemCandidateId,
    pub system_spec: SystemSpecId,
    pub immutable_flake_root: PathBuf,
    pub canonical_system_spec: String,
    pub translation_evidence: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProducedCandidateManifest {
    pub manifest: TrustedMaterializationCandidate,
    pub manifest_path: PathBuf,
    pub receipt: CandidateManifestReceipt,
    pub receipt_path: PathBuf,
    pub source_gcroot: PathBuf,
    pub causal_record: CausalRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateProducerError {
    InvalidSender,
    SystemSpec(CanonicalSystemSpecError),
    Backend(NixBackendError),
    Source(String),
    InvalidManifestRoot,
    InvalidReceiptRoot,
    InvalidSourceGcRoot,
    ManifestConflict,
    ReceiptConflict,
    SourceGcRootConflict,
    Clock(String),
    RandomSource(String),
    InvalidCreatedManifest,
    InvalidCreatedReceipt,
    Io(String),
}

pub trait CandidateSourceBuilder {
    fn build_immutable_source(
        &self,
        spec: &SystemSpec,
        translation: &NixTranslation,
    ) -> Result<PathBuf, String>;
}

pub struct StdNixCandidateSourceBuilder {
    nix_program: PathBuf,
    nixpkgs_source: PathBuf,
    base_module_source: PathBuf,
    staging_root: PathBuf,
    expected_owner_uid: u32,
}

impl StdNixCandidateSourceBuilder {
    pub fn new(
        nix_program: impl Into<PathBuf>,
        nixpkgs_source: impl Into<PathBuf>,
        base_module_source: impl Into<PathBuf>,
        staging_root: impl Into<PathBuf>,
        expected_owner_uid: u32,
    ) -> Self {
        Self {
            nix_program: nix_program.into(),
            nixpkgs_source: nixpkgs_source.into(),
            base_module_source: base_module_source.into(),
            staging_root: staging_root.into(),
            expected_owner_uid,
        }
    }

    fn run(&self, args: &[String]) -> Result<Output, String> {
        Command::new(&self.nix_program)
            .args(args)
            .stdin(Stdio::null())
            .env_clear()
            .env("HOME", "/root")
            .env("USER", "root")
            .env("LOGNAME", "root")
            .env("LANG", "C")
            .output()
            .map_err(|error| error.to_string())
    }
}

impl CandidateSourceBuilder for StdNixCandidateSourceBuilder {
    fn build_immutable_source(
        &self,
        spec: &SystemSpec,
        translation: &NixTranslation,
    ) -> Result<PathBuf, String> {
        validate_canonical_store_subpath(&self.nixpkgs_source)?;
        validate_canonical_store_subpath(&self.base_module_source)?;
        validate_directory(&self.staging_root, self.expected_owner_uid)?;

        let base_module = fs::read_to_string(&self.base_module_source)
            .map_err(|error| format!("failed to read trusted base module: {error}"))?;
        let nonce = random_hex_128()?;
        let work = self.staging_root.join(format!("candidate-{nonce}"));
        let staging = work.join(CANDIDATE_SOURCE_NAME);
        fs::create_dir(&work)
            .map_err(|error| format!("failed to create source work directory: {error}"))?;
        fs::set_permissions(&work, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("failed to protect source work directory: {error}"))?;
        fs::create_dir(&staging)
            .map_err(|error| format!("failed to create source staging directory: {error}"))?;
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("failed to set source staging permissions: {error}"))?;

        let result = (|| {
            write_file(&staging.join("base.nix"), &base_module, 0o644)?;
            write_file(
                &staging.join("generated.nix"),
                &translation.module_text,
                0o644,
            )?;
            let flake = canonical_candidate_flake(spec, &self.nixpkgs_source)?;
            write_file(&staging.join("flake.nix"), &flake, 0o644)?;

            let output = self.run(&[
                "store".into(),
                "add".into(),
                staging.display().to_string(),
            ])?;
            if !output.status.success() {
                return Err(format!(
                    "nix store add failed with {:?}: {}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            let source = one_store_path(&String::from_utf8_lossy(&output.stdout))?;
            validate_canonical_store_subpath(&source)?;
            Ok(source)
        })();

        let _ = fs::remove_dir_all(&work);
        result
    }
}

pub struct RootSystemSpecCandidateProducer {
    manifest_root: PathBuf,
    receipt_root: PathBuf,
    source_gcroot_root: PathBuf,
    expected_owner_uid: u32,
}

impl RootSystemSpecCandidateProducer {
    pub fn production_default() -> Self {
        Self::new(
            DEFAULT_TRUSTED_CANDIDATE_ROOT,
            DEFAULT_CANDIDATE_RECEIPT_ROOT,
            DEFAULT_CANDIDATE_SOURCE_GCROOT_ROOT,
            0,
        )
    }

    pub fn new(
        manifest_root: impl Into<PathBuf>,
        receipt_root: impl Into<PathBuf>,
        source_gcroot_root: impl Into<PathBuf>,
        expected_owner_uid: u32,
    ) -> Self {
        Self {
            manifest_root: manifest_root.into(),
            receipt_root: receipt_root.into(),
            source_gcroot_root: source_gcroot_root.into(),
            expected_owner_uid,
        }
    }

    /// Validate a backend-neutral SystemSpec and derive every native identity in root.
    ///
    /// The requester controls only semantic SystemSpec state. Candidate id, manifest id,
    /// immutable source, installable attribute, translation, timestamp and causal receipt
    /// are generated or recomputed inside this boundary.
    pub fn produce<B: CandidateSourceBuilder>(
        &self,
        requester_system_bus_name: &str,
        canonical_spec_text: &str,
        source_builder: &B,
    ) -> Result<ProducedCandidateManifest, CandidateProducerError> {
        if !requester_system_bus_name.starts_with(':') {
            return Err(CandidateProducerError::InvalidSender);
        }
        self.validate_layout()?;

        let spec = parse_canonical_system_spec(canonical_spec_text)
            .map_err(CandidateProducerError::SystemSpec)?;
        let translation = NixOsBackend::translate(&spec).map_err(CandidateProducerError::Backend)?;
        let source = source_builder
            .build_immutable_source(&spec, &translation)
            .map_err(CandidateProducerError::Source)?;
        validate_canonical_store_subpath(&source).map_err(CandidateProducerError::Source)?;

        let nonce = random_hex_128().map_err(CandidateProducerError::RandomSource)?;
        let manifest_id = format!("manifest:systemspec-{nonce}");
        let candidate = SystemCandidateId::from(format!("candidate:systemspec-{nonce}"));
        let causal_id = CausalRecordId::from(format!("causal:candidate-manifest-{nonce}"));
        let occurred_at_unix_ms = now_unix_ms().map_err(CandidateProducerError::Clock)?;
        let installable_attribute = format!(
            "nixosConfigurations.{}.config.system.build.toplevel",
            spec.hostname
        );

        let translation_evidence = translation
            .trace
            .iter()
            .map(|step| format!("{} -> {}", step.semantic_source, step.nix_target))
            .collect::<Vec<_>>();
        let mut manifest_provenance = vec![
            "producer:systemspec-nixos-v1".into(),
            format!("causal-record:{causal_id}"),
            format!("requester-system-bus:{requester_system_bus_name}"),
            format!("system-spec:{}", spec.id),
            "system-spec-validation:passed".into(),
            "nix-translation:deterministic".into(),
            format!("immutable-source:{}", source.display()),
        ];
        manifest_provenance.extend(
            translation_evidence
                .iter()
                .map(|line| format!("translation:{line}")),
        );

        let manifest = TrustedMaterializationCandidate {
            manifest_id: manifest_id.clone(),
            candidate: candidate.clone(),
            system_spec: spec.id.clone(),
            immutable_flake_root: source.clone(),
            installable_attribute,
            provenance: manifest_provenance.clone(),
        };
        let receipt = CandidateManifestReceipt {
            causal_id: causal_id.clone(),
            occurred_at_unix_ms,
            requester_system_bus_name: requester_system_bus_name.to_owned(),
            manifest_id: manifest_id.clone(),
            candidate: candidate.clone(),
            system_spec: spec.id.clone(),
            immutable_flake_root: source.clone(),
            canonical_system_spec: canonical_spec_text.to_owned(),
            translation_evidence: translation_evidence.clone(),
        };
        let causal_record = CausalRecord {
            id: causal_id,
            kind: CausalKind::Custom("system-candidate-manifest-produced".into()),
            occurred_at_unix_ms,
            parents: Vec::new(),
            actor: PRODUCER_ACTOR.into(),
            summary: format!("Produced trusted candidate manifest {manifest_id}"),
            why: "validated canonical SystemSpec and deterministic NixOS translation".into(),
            event: None,
            situation: None,
            task: None,
            requirement_graph: None,
            binding_plan: None,
            binding_lease: None,
            improvement_proposal: None,
            evidence: manifest_provenance,
            expected_effects: vec![
                "create immutable candidate source without activating the live system".into(),
                "publish a root-owned trusted materialization manifest".into(),
            ],
            actual_effects: vec![
                format!("candidate-source:{}", source.display()),
                format!("candidate-manifest:{manifest_id}"),
            ],
            authorization: Some("validated semantic SystemSpec; materialization-only".into()),
            rollback_reference: None,
        };

        let source_gcroot = self.retain_source(&manifest_id, &source)?;
        let manifest_path = self.manifest_path(&manifest_id);
        let receipt_path = self.receipt_path(&manifest_id);

        if path_exists(&manifest_path).map_err(CandidateProducerError::Io)? {
            let _ = fs::remove_file(&source_gcroot);
            return Err(CandidateProducerError::ManifestConflict);
        }
        if path_exists(&receipt_path).map_err(CandidateProducerError::Io)? {
            let _ = fs::remove_file(&source_gcroot);
            return Err(CandidateProducerError::ReceiptConflict);
        }

        if let Err(error) = create_protected_file(
            &manifest_path,
            &canonical_trusted_candidate(&manifest),
        ) {
            let _ = fs::remove_file(&source_gcroot);
            return Err(CandidateProducerError::Io(error));
        }
        if let Err(error) = create_protected_file(
            &receipt_path,
            &canonical_candidate_receipt(&receipt),
        ) {
            let _ = fs::remove_file(&manifest_path);
            let _ = fs::remove_file(&source_gcroot);
            return Err(CandidateProducerError::Io(error));
        }

        validate_root_file(&manifest_path, self.expected_owner_uid)
            .map_err(|_| CandidateProducerError::InvalidCreatedManifest)?;
        validate_root_file(&receipt_path, self.expected_owner_uid)
            .map_err(|_| CandidateProducerError::InvalidCreatedReceipt)?;
        sync_dir(&self.manifest_root).map_err(CandidateProducerError::Io)?;
        sync_dir(&self.receipt_root).map_err(CandidateProducerError::Io)?;
        sync_dir(&self.source_gcroot_root).map_err(CandidateProducerError::Io)?;

        Ok(ProducedCandidateManifest {
            manifest,
            manifest_path,
            receipt,
            receipt_path,
            source_gcroot,
            causal_record,
        })
    }

    fn validate_layout(&self) -> Result<(), CandidateProducerError> {
        validate_directory(&self.manifest_root, self.expected_owner_uid)
            .map_err(|_| CandidateProducerError::InvalidManifestRoot)?;
        validate_directory(&self.receipt_root, self.expected_owner_uid)
            .map_err(|_| CandidateProducerError::InvalidReceiptRoot)?;
        validate_directory(&self.source_gcroot_root, self.expected_owner_uid)
            .map_err(|_| CandidateProducerError::InvalidSourceGcRoot)?;
        Ok(())
    }

    fn manifest_path(&self, manifest_id: &str) -> PathBuf {
        self.manifest_root
            .join(format!("manifest-{}.candidate", hex_text(manifest_id)))
    }

    fn receipt_path(&self, manifest_id: &str) -> PathBuf {
        self.receipt_root
            .join(format!("manifest-{}.receipt", hex_text(manifest_id)))
    }

    fn source_gcroot_path(&self, manifest_id: &str) -> PathBuf {
        self.source_gcroot_root
            .join(format!("manifest-{}-source", hex_text(manifest_id)))
    }

    fn retain_source(
        &self,
        manifest_id: &str,
        source: &Path,
    ) -> Result<PathBuf, CandidateProducerError> {
        let root = self.source_gcroot_path(manifest_id);
        match fs::symlink_metadata(&root) {
            Ok(_) => return Err(CandidateProducerError::SourceGcRootConflict),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(CandidateProducerError::Io(error.to_string())),
        }
        symlink(source, &root).map_err(|error| CandidateProducerError::Io(error.to_string()))?;
        Ok(root)
    }
}

pub fn canonical_system_spec(spec: &SystemSpec) -> String {
    let mut features = spec.profile.features.clone();
    features.sort_by(|left, right| left.feature.as_str().cmp(right.feature.as_str()));

    let mut lines = vec![
        "theblob-system-spec-v1".to_owned(),
        format!("system-spec={}", hex_text(spec.id.as_str())),
        format!("hostname={}", hex_text(&spec.hostname)),
        format!("architecture={}", architecture_name(&spec.architecture)),
        format!("base-channel={}", channel_name(&spec.base_channel)),
        format!("kernel-policy={}", kernel_name(&spec.kernel_policy)),
        format!("profile={}", hex_text(spec.profile.id.as_str())),
        format!(
            "construction-mode={}",
            construction_name(&spec.profile.construction_mode)
        ),
        format!("priority-count={}", spec.profile.priorities.len()),
    ];
    for (index, priority) in spec.profile.priorities.iter().enumerate() {
        lines.push(format!("priority-{index}={}", priority_name(priority)));
    }
    lines.push(format!("feature-count={}", features.len()));
    for (index, feature) in features.iter().enumerate() {
        lines.push(format!(
            "feature-{index}-id={}",
            hex_text(feature.feature.as_str())
        ));
        lines.push(format!(
            "feature-{index}-state={}",
            feature_state_name(&feature.state)
        ));
    }
    lines.push(format!(
        "experience-profile={}",
        spec.experience_profile
            .as_ref()
            .map(|value| hex_text(value.as_str()))
            .unwrap_or_default()
    ));
    lines.push(String::new());
    lines.join("\n")
}

pub fn parse_canonical_system_spec(text: &str) -> Result<SystemSpec, CanonicalSystemSpecError> {
    if text.len() as u64 > MAX_CANONICAL_SYSTEM_SPEC_BYTES {
        return Err(CanonicalSystemSpecError::TooLarge);
    }
    let mut cursor = SpecCursor::new(text);
    cursor.literal("theblob-system-spec-v1")?;
    let system_spec = cursor.hex_field("system-spec")?;
    let hostname = cursor.hex_field("hostname")?;
    let architecture = parse_architecture(cursor.field("architecture")?)?;
    let base_channel = parse_channel(cursor.field("base-channel")?)?;
    let kernel_policy = parse_kernel(cursor.field("kernel-policy")?)?;
    let profile = cursor.hex_field("profile")?;
    let construction_mode = parse_construction(cursor.field("construction-mode")?)?;
    let priority_count = cursor.count_field("priority-count", 32)?;
    let mut priorities = Vec::with_capacity(priority_count);
    for index in 0..priority_count {
        priorities.push(parse_priority(cursor.field(&format!("priority-{index}"))?)?);
    }
    let feature_count = cursor.count_field("feature-count", 256)?;
    let mut features = Vec::with_capacity(feature_count);
    for index in 0..feature_count {
        let id = cursor.hex_field(&format!("feature-{index}-id"))?;
        let state = parse_feature_state(cursor.field(&format!("feature-{index}-state"))?)?;
        validate_id(&id)?;
        features.push(SystemFeatureSelection {
            feature: SystemFeatureId::from(id),
            state,
        });
    }
    let experience = cursor.hex_field("experience-profile")?;
    cursor.finish()?;

    validate_id(&system_spec)?;
    validate_id(&profile)?;
    if !experience.is_empty() {
        validate_id(&experience)?;
    }
    let spec = SystemSpec {
        id: SystemSpecId::from(system_spec),
        hostname,
        architecture,
        base_channel,
        kernel_policy,
        profile: SemanticBuildProfile {
            id: SystemProfileId::from(profile),
            construction_mode,
            priorities,
            features,
        },
        experience_profile: if experience.is_empty() {
            None
        } else {
            Some(ExperienceProfileId::from(experience))
        },
    };
    spec.validate()
        .map_err(CanonicalSystemSpecError::InvalidSystemSpec)?;
    if canonical_system_spec(&spec) != text {
        return Err(CanonicalSystemSpecError::NonCanonical);
    }
    Ok(spec)
}

pub fn canonical_candidate_receipt(receipt: &CandidateManifestReceipt) -> String {
    let mut lines = vec![
        "theblob-candidate-manifest-receipt-v1".to_owned(),
        format!("causal-id={}", hex_text(receipt.causal_id.as_str())),
        format!("occurred-at-unix-ms={}", receipt.occurred_at_unix_ms),
        format!(
            "requester-system-bus={}",
            hex_text(&receipt.requester_system_bus_name)
        ),
        format!("manifest-id={}", hex_text(&receipt.manifest_id)),
        format!("candidate={}", hex_text(receipt.candidate.as_str())),
        format!("system-spec={}", hex_text(receipt.system_spec.as_str())),
        format!(
            "immutable-flake-root={}",
            hex_text(&receipt.immutable_flake_root.display().to_string())
        ),
        format!(
            "canonical-system-spec={}",
            hex_text(&receipt.canonical_system_spec)
        ),
        format!(
            "translation-evidence-count={}",
            receipt.translation_evidence.len()
        ),
    ];
    for (index, evidence) in receipt.translation_evidence.iter().enumerate() {
        lines.push(format!(
            "translation-evidence-{index}={}",
            hex_text(evidence)
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn canonical_candidate_flake(spec: &SystemSpec, nixpkgs_source: &Path) -> Result<String, String> {
    let system = match spec.architecture {
        SystemArchitecture::X86_64 => "x86_64-linux",
        SystemArchitecture::Aarch64 => "aarch64-linux",
    };
    let nixpkgs = nix_string(&nixpkgs_source.display().to_string());
    let hostname = nix_string(&spec.hostname);
    Ok(format!(
        "{{\n  inputs.nixpkgs.url = \"path:{nixpkgs}\";\n\n  outputs = {{ self, nixpkgs }}: {{\n    nixosConfigurations.\"{hostname}\" = nixpkgs.lib.nixosSystem {{\n      system = \"{system}\";\n      modules = [ ./base.nix ./generated.nix ];\n    }};\n  }};\n}}\n"
    ))
}

struct SpecCursor<'a> {
    lines: Vec<&'a str>,
    position: usize,
}

impl<'a> SpecCursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.split('\n').collect(),
            position: 0,
        }
    }

    fn next(&mut self) -> Result<&'a str, CanonicalSystemSpecError> {
        let value = self
            .lines
            .get(self.position)
            .copied()
            .ok_or(CanonicalSystemSpecError::Malformed)?;
        self.position += 1;
        Ok(value)
    }

    fn literal(&mut self, expected: &str) -> Result<(), CanonicalSystemSpecError> {
        if self.next()? != expected {
            return Err(CanonicalSystemSpecError::Malformed);
        }
        Ok(())
    }

    fn field(&mut self, key: &str) -> Result<&'a str, CanonicalSystemSpecError> {
        self.next()?
            .strip_prefix(&format!("{key}="))
            .ok_or(CanonicalSystemSpecError::Malformed)
    }

    fn hex_field(&mut self, key: &str) -> Result<String, CanonicalSystemSpecError> {
        decode_hex(self.field(key)?)
    }

    fn count_field(
        &mut self,
        key: &str,
        maximum: usize,
    ) -> Result<usize, CanonicalSystemSpecError> {
        let value = self
            .field(key)?
            .parse::<usize>()
            .map_err(|_| CanonicalSystemSpecError::Malformed)?;
        if value > maximum {
            return Err(CanonicalSystemSpecError::Malformed);
        }
        Ok(value)
    }

    fn finish(&mut self) -> Result<(), CanonicalSystemSpecError> {
        if !self.next()?.is_empty() || self.position != self.lines.len() {
            return Err(CanonicalSystemSpecError::Malformed);
        }
        Ok(())
    }
}

fn parse_architecture(value: &str) -> Result<SystemArchitecture, CanonicalSystemSpecError> {
    match value {
        "x86_64" => Ok(SystemArchitecture::X86_64),
        "aarch64" => Ok(SystemArchitecture::Aarch64),
        _ => Err(CanonicalSystemSpecError::Malformed),
    }
}

fn parse_channel(value: &str) -> Result<SystemChannel, CanonicalSystemSpecError> {
    match value {
        "stable" => Ok(SystemChannel::Stable),
        "testing" => Ok(SystemChannel::Testing),
        "edge" => Ok(SystemChannel::Edge),
        _ => Err(CanonicalSystemSpecError::Malformed),
    }
}

fn parse_kernel(value: &str) -> Result<KernelPolicy, CanonicalSystemSpecError> {
    match value {
        "distribution-default" => Ok(KernelPolicy::DistributionDefault),
        "latest-supported" => Ok(KernelPolicy::LatestSupported),
        _ => Err(CanonicalSystemSpecError::Malformed),
    }
}

fn parse_construction(value: &str) -> Result<SystemConstructionMode, CanonicalSystemSpecError> {
    match value {
        "ready" => Ok(SystemConstructionMode::Ready),
        "ai-designed" => Ok(SystemConstructionMode::AiDesigned),
        "expert" => Ok(SystemConstructionMode::Expert),
        _ => Err(CanonicalSystemSpecError::Malformed),
    }
}

fn parse_priority(value: &str) -> Result<SystemPriority, CanonicalSystemSpecError> {
    match value {
        "reliability" => Ok(SystemPriority::Reliability),
        "security" => Ok(SystemPriority::Security),
        "latency" => Ok(SystemPriority::Latency),
        "energy" => Ok(SystemPriority::Energy),
        "memory" => Ok(SystemPriority::Memory),
        "build-time" => Ok(SystemPriority::BuildTime),
        _ => Err(CanonicalSystemSpecError::Malformed),
    }
}

fn parse_feature_state(value: &str) -> Result<FeatureState, CanonicalSystemSpecError> {
    match value {
        "enabled" => Ok(FeatureState::Enabled),
        "disabled" => Ok(FeatureState::Disabled),
        _ => Err(CanonicalSystemSpecError::Malformed),
    }
}

fn architecture_name(value: &SystemArchitecture) -> &'static str {
    match value {
        SystemArchitecture::X86_64 => "x86_64",
        SystemArchitecture::Aarch64 => "aarch64",
    }
}

fn channel_name(value: &SystemChannel) -> &'static str {
    match value {
        SystemChannel::Stable => "stable",
        SystemChannel::Testing => "testing",
        SystemChannel::Edge => "edge",
    }
}

fn kernel_name(value: &KernelPolicy) -> &'static str {
    match value {
        KernelPolicy::DistributionDefault => "distribution-default",
        KernelPolicy::LatestSupported => "latest-supported",
    }
}

fn construction_name(value: &SystemConstructionMode) -> &'static str {
    match value {
        SystemConstructionMode::Ready => "ready",
        SystemConstructionMode::AiDesigned => "ai-designed",
        SystemConstructionMode::Expert => "expert",
    }
}

fn priority_name(value: &SystemPriority) -> &'static str {
    match value {
        SystemPriority::Reliability => "reliability",
        SystemPriority::Security => "security",
        SystemPriority::Latency => "latency",
        SystemPriority::Energy => "energy",
        SystemPriority::Memory => "memory",
        SystemPriority::BuildTime => "build-time",
    }
}

fn feature_state_name(value: &FeatureState) -> &'static str {
    match value {
        FeatureState::Enabled => "enabled",
        FeatureState::Disabled => "disabled",
    }
}

fn validate_id(value: &str) -> Result<(), CanonicalSystemSpecError> {
    if value.is_empty() || value.len() > 256 {
        return Err(CanonicalSystemSpecError::InvalidId);
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_' | b'.'))
    {
        return Err(CanonicalSystemSpecError::InvalidId);
    }
    Ok(())
}

fn validate_directory(path: &Path, expected_owner_uid: u32) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("invalid protected directory".into());
    }
    Ok(())
}

fn validate_root_file(path: &Path, expected_owner_uid: u32) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != expected_owner_uid
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err("invalid protected file".into());
    }
    Ok(())
}

fn validate_store_subpath(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("path is not absolute".into());
    }
    let components = path.components().collect::<Vec<_>>();
    if components.len() < 4
        || components[0] != Component::RootDir
        || components[1] != Component::Normal("nix".as_ref())
        || components[2] != Component::Normal("store".as_ref())
        || components[3].as_os_str().is_empty()
        || components[3..]
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("path is outside an immutable Nix store object".into());
    }
    Ok(())
}

fn validate_canonical_store_subpath(path: &Path) -> Result<(), String> {
    validate_store_subpath(path)?;
    let canonical = fs::canonicalize(path).map_err(|error| error.to_string())?;
    if canonical != path {
        return Err(format!(
            "store path is not canonical: {} -> {}",
            path.display(),
            canonical.display()
        ));
    }
    validate_store_subpath(&canonical)
}

fn one_store_path(text: &str) -> Result<PathBuf, String> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err(format!("expected one store path, observed {lines:?}"));
    }
    let path = PathBuf::from(lines[0]);
    validate_store_subpath(&path)?;
    Ok(path)
}

fn write_file(path: &Path, text: &str, mode: u32) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| error.to_string())
}

fn create_protected_file(path: &Path, text: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| error.to_string())
}

fn sync_dir(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn nix_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace("${", "\\${")
}

fn now_unix_ms() -> Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    u64::try_from(elapsed.as_millis()).map_err(|_| "system clock overflow".to_owned())
}

fn random_hex_128() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| error.to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn hex_text(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_hex(value: &str) -> Result<String, CanonicalSystemSpecError> {
    if value.len() % 2 != 0 {
        return Err(CanonicalSystemSpecError::Malformed);
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = nibble(pair[0])?;
        let low = nibble(pair[1])?;
        bytes.push((high << 4) | low);
    }
    String::from_utf8(bytes).map_err(|_| CanonicalSystemSpecError::Malformed)
}

fn nibble(value: u8) -> Result<u8, CanonicalSystemSpecError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(CanonicalSystemSpecError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> SystemSpec {
        SystemSpec {
            id: SystemSpecId::from("system:linux-pilot"),
            hostname: "blob-pilot".into(),
            architecture: SystemArchitecture::X86_64,
            base_channel: SystemChannel::Stable,
            kernel_policy: KernelPolicy::LatestSupported,
            profile: SemanticBuildProfile {
                id: SystemProfileId::from("profile:development-balanced"),
                construction_mode: SystemConstructionMode::AiDesigned,
                priorities: vec![SystemPriority::Reliability, SystemPriority::Energy],
                features: vec![
                    SystemFeatureSelection::enabled("bluetooth"),
                    SystemFeatureSelection::enabled("hyprland"),
                    SystemFeatureSelection::enabled("pipewire"),
                    SystemFeatureSelection::disabled("printing"),
                ],
            },
            experience_profile: Some(ExperienceProfileId::from("experience:hyprland")),
        }
    }

    #[test]
    fn canonical_system_spec_round_trips() {
        let expected = spec();
        let text = canonical_system_spec(&expected);
        assert_eq!(parse_canonical_system_spec(&text), Ok(expected));
    }

    #[test]
    fn feature_order_is_canonicalized() {
        let mut unordered = spec();
        unordered.profile.features.reverse();
        let text = canonical_system_spec(&unordered);
        let bluetooth = text.find("626c7565746f6f7468").expect("bluetooth");
        let hyprland = text.find("687970726c616e64").expect("hyprland");
        let pipewire = text.find("7069706577697265").expect("pipewire");
        let printing = text.find("7072696e74696e67").expect("printing");
        assert!(bluetooth < hyprland && hyprland < pipewire && pipewire < printing);
        assert_eq!(parse_canonical_system_spec(&text), Ok(spec()));
    }

    #[test]
    fn extra_native_configuration_is_rejected() {
        let mut text = canonical_system_spec(&spec());
        text.push_str("raw-nix=696d70757265\n");
        assert!(parse_canonical_system_spec(&text).is_err());
    }

    #[test]
    fn unsafe_ids_are_rejected() {
        assert_eq!(
            validate_id("system:ok;--impure"),
            Err(CanonicalSystemSpecError::InvalidId)
        );
    }

    #[test]
    fn canonical_flake_uses_only_trusted_inputs_and_generated_modules() {
        let flake = canonical_candidate_flake(
            &spec(),
            Path::new("/nix/store/aaaaaaaa-nixpkgs-source"),
        )
        .expect("flake");
        assert!(flake.contains("modules = [ ./base.nix ./generated.nix ];"));
        assert!(flake.contains("path:/nix/store/aaaaaaaa-nixpkgs-source"));
        assert!(!flake.contains("builtins.getEnv"));
        assert!(!flake.contains("--impure"));
    }
}
