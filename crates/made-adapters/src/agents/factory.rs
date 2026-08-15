//! Dispatching [`AgentFactoryPort`] across provider adapters.
//!
//! The composition root wires one instance of this factory; it
//! recognises `kind == "noop"` unconditionally plus any provider whose
//! Cargo feature is enabled **and** whose credentials are present in
//! the environment at boot. Per-instance overrides (model, endpoint,
//! max_tokens) come from the descriptor's `attributes`; credentials
//! never live in the descriptor because descriptors are persisted in
//! Postgres.
//!
//! Environment variables consumed by [`DispatchingAgentFactory::from_env`]:
//!
//! - `MADE_ANTHROPIC_API_KEY` (required to enable kind=`anthropic`)
//! - `MADE_ANTHROPIC_MODEL`, `MADE_ANTHROPIC_ENDPOINT`,
//!   `MADE_ANTHROPIC_MAX_TOKENS` (optional overrides of adapter defaults)
//! - `MADE_OPENAI_API_KEY` (required to enable kind=`openai`)
//! - `MADE_OPENAI_MODEL`, `MADE_OPENAI_ENDPOINT`,
//!   `MADE_OPENAI_MAX_TOKENS` (optional)
//! - `MADE_VLLM_MODEL` and `MADE_VLLM_ENDPOINT` (both required to
//!   enable kind=`vllm`); `MADE_VLLM_BEARER_TOKEN`,
//!   `MADE_VLLM_MAX_TOKENS`, `MADE_VLLM_TIMEOUT_SECS` (optional)
//!
//! Per-descriptor attributes recognised on `create`:
//!
//! - `provider.model` (string)
//! - `provider.endpoint` (string)
//! - `provider.max_tokens` (number, ≤ `u32::MAX`)
//! - `provider.timeout_secs` (number, ≤ `u32::MAX`; vLLM only)

use std::fmt;
use std::sync::Arc;
#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
use std::time::Duration;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{
    AgentDescriptor, AgentFactoryPort, AgentPort, MetricsRecorderPort, NoopMetricsRecorder,
};
#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
use made_core::value_objects::Attributes;

use crate::noop::{NoopAgent, NOOP_AGENT_KIND};

#[cfg(feature = "agent-anthropic")]
use super::anthropic::{AnthropicAgent, AnthropicApiKey, AnthropicConfig};

#[cfg(feature = "agent-openai")]
use super::openai::{OpenAiAgent, OpenAiApiKey, OpenAiConfig};

#[cfg(feature = "agent-vllm")]
use super::vllm::{VllmAgent, VllmBearerToken, VllmConfig};

pub const ANTHROPIC_AGENT_KIND: &str = "anthropic";
pub const OPENAI_AGENT_KIND: &str = "openai";
pub const VLLM_AGENT_KIND: &str = "vllm";

#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
const ATTR_MODEL: &str = "provider.model";
#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
const ATTR_ENDPOINT: &str = "provider.endpoint";
#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
const ATTR_MAX_TOKENS: &str = "provider.max_tokens";
#[cfg(feature = "agent-vllm")]
const ATTR_TIMEOUT_SECS: &str = "provider.timeout_secs";

/// Composes provider-specific factories behind a single
/// [`AgentFactoryPort`].
///
/// Built via [`DispatchingAgentFactory::from_env`] in production; the
/// `with_*` builders are kept for tests and bespoke composition.
#[derive(Clone)]
pub struct DispatchingAgentFactory {
    #[cfg(feature = "agent-anthropic")]
    anthropic: Option<AnthropicConfig>,
    #[cfg(feature = "agent-openai")]
    openai: Option<OpenAiConfig>,
    #[cfg(feature = "agent-vllm")]
    vllm: Option<VllmConfig>,
    /// Recorder forwarded to every agent this factory materializes, so
    /// provider failures are counted. Defaults to a no-op.
    metrics: Arc<dyn MetricsRecorderPort>,
}

impl Default for DispatchingAgentFactory {
    fn default() -> Self {
        Self {
            #[cfg(feature = "agent-anthropic")]
            anthropic: None,
            #[cfg(feature = "agent-openai")]
            openai: None,
            #[cfg(feature = "agent-vllm")]
            vllm: None,
            metrics: Arc::new(NoopMetricsRecorder),
        }
    }
}

impl fmt::Debug for DispatchingAgentFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DispatchingAgentFactory")
            .field("supported_kinds", &self.supported_kinds())
            .finish()
    }
}

impl DispatchingAgentFactory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a metrics recorder, forwarded to every agent this factory
    /// materializes. The composition root wires the real recorder; the
    /// default no-op keeps tests and minimal builds free of one.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = metrics;
        self
    }

    /// Read provider configurations from `MADE_*` env vars.
    ///
    /// A provider arm is **enabled** iff (a) its Cargo feature is
    /// compiled in and (b) its required env var(s) are set. Missing
    /// optional overrides fall back to the adapter's defaults. Empty
    /// strings count as unset so operators can clear a variable without
    /// deleting it.
    pub fn from_env() -> Result<Self, DomainError> {
        #[cfg(not(any(
            feature = "agent-anthropic",
            feature = "agent-openai",
            feature = "agent-vllm"
        )))]
        {
            Ok(Self::new())
        }

        #[cfg(any(
            feature = "agent-anthropic",
            feature = "agent-openai",
            feature = "agent-vllm"
        ))]
        {
            let mut factory = Self::new();

            #[cfg(feature = "agent-anthropic")]
            {
                factory.anthropic = load_anthropic_from_env()?;
            }
            #[cfg(feature = "agent-openai")]
            {
                factory.openai = load_openai_from_env()?;
            }
            #[cfg(feature = "agent-vllm")]
            {
                factory.vllm = load_vllm_from_env()?;
            }

            Ok(factory)
        }
    }

    #[cfg(feature = "agent-anthropic")]
    #[must_use]
    pub fn with_anthropic(mut self, config: AnthropicConfig) -> Self {
        self.anthropic = Some(config);
        self
    }

    #[cfg(feature = "agent-openai")]
    #[must_use]
    pub fn with_openai(mut self, config: OpenAiConfig) -> Self {
        self.openai = Some(config);
        self
    }

    #[cfg(feature = "agent-vllm")]
    #[must_use]
    pub fn with_vllm(mut self, config: VllmConfig) -> Self {
        self.vllm = Some(config);
        self
    }

    /// Kinds the operator can register on this binary.
    ///
    /// `"noop"` is always present. A provider kind appears only when
    /// its feature is compiled in AND a base config was supplied (via
    /// env or builder). Useful for honest startup logging.
    #[must_use]
    pub fn supported_kinds(&self) -> Vec<&'static str> {
        #[cfg(not(any(
            feature = "agent-anthropic",
            feature = "agent-openai",
            feature = "agent-vllm"
        )))]
        {
            vec![NOOP_AGENT_KIND]
        }

        #[cfg(any(
            feature = "agent-anthropic",
            feature = "agent-openai",
            feature = "agent-vllm"
        ))]
        {
            let mut kinds = vec![NOOP_AGENT_KIND];
            #[cfg(feature = "agent-anthropic")]
            if self.anthropic.is_some() {
                kinds.push(ANTHROPIC_AGENT_KIND);
            }
            #[cfg(feature = "agent-openai")]
            if self.openai.is_some() {
                kinds.push(OPENAI_AGENT_KIND);
            }
            #[cfg(feature = "agent-vllm")]
            if self.vllm.is_some() {
                kinds.push(VLLM_AGENT_KIND);
            }
            kinds
        }
    }
}

#[async_trait]
impl AgentFactoryPort for DispatchingAgentFactory {
    async fn create(&self, descriptor: AgentDescriptor) -> Result<Arc<dyn AgentPort>, DomainError> {
        match descriptor.kind.as_str() {
            NOOP_AGENT_KIND => Ok(Arc::new(NoopAgent::new(
                descriptor.id,
                descriptor.specialty,
            ))),

            #[cfg(feature = "agent-anthropic")]
            ANTHROPIC_AGENT_KIND => {
                let base = self
                    .anthropic
                    .as_ref()
                    .ok_or(DomainError::InvariantViolated {
                        reason: "agent factory: anthropic kind requested but not configured",
                    })?;
                let config = apply_anthropic_attributes(base.clone(), &descriptor.attributes)?;
                Ok(Arc::new(
                    AnthropicAgent::new(descriptor.id, descriptor.specialty, config)?
                        .with_metrics(self.metrics.clone()),
                ))
            }

            #[cfg(feature = "agent-openai")]
            OPENAI_AGENT_KIND => {
                let base = self.openai.as_ref().ok_or(DomainError::InvariantViolated {
                    reason: "agent factory: openai kind requested but not configured",
                })?;
                let config = apply_openai_attributes(base.clone(), &descriptor.attributes)?;
                Ok(Arc::new(
                    OpenAiAgent::new(descriptor.id, descriptor.specialty, config)?
                        .with_metrics(self.metrics.clone()),
                ))
            }

            #[cfg(feature = "agent-vllm")]
            VLLM_AGENT_KIND => {
                let base = self.vllm.as_ref().ok_or(DomainError::InvariantViolated {
                    reason: "agent factory: vllm kind requested but not configured",
                })?;
                let config = apply_vllm_attributes(base.clone(), &descriptor.attributes)?;
                Ok(Arc::new(
                    VllmAgent::new(descriptor.id, descriptor.specialty, config)?
                        .with_metrics(self.metrics.clone()),
                ))
            }

            _ => Err(DomainError::InvariantViolated {
                reason: "agent factory: unsupported agent kind",
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Env helpers
// ---------------------------------------------------------------------------

#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
fn env_u32(key: &str, field: &'static str) -> Result<Option<u32>, DomainError> {
    let Some(raw) = env_nonempty(key) else {
        return Ok(None);
    };
    raw.parse::<u32>()
        .map(Some)
        .map_err(|_| DomainError::InvariantViolated { reason: field })
}

#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
fn env_duration_secs(key: &str, field: &'static str) -> Result<Option<Duration>, DomainError> {
    let Some(raw) = env_nonempty(key) else {
        return Ok(None);
    };
    let secs = raw
        .parse::<u64>()
        .map_err(|_| DomainError::InvariantViolated { reason: field })?;
    if secs == 0 {
        return Err(DomainError::MustBeNonZero { field });
    }
    Ok(Some(Duration::from_secs(secs)))
}

#[cfg(feature = "agent-anthropic")]
fn load_anthropic_from_env() -> Result<Option<AnthropicConfig>, DomainError> {
    let Some(api_key_raw) = env_nonempty("MADE_ANTHROPIC_API_KEY") else {
        return Ok(None);
    };
    let api_key = AnthropicApiKey::new(api_key_raw)?;
    let mut config = AnthropicConfig::new(api_key);
    if let Some(model) = env_nonempty("MADE_ANTHROPIC_MODEL") {
        config = config.with_model(model)?;
    }
    if let Some(endpoint) = env_nonempty("MADE_ANTHROPIC_ENDPOINT") {
        config = config.with_endpoint(endpoint)?;
    }
    if let Some(max_tokens) = env_u32("MADE_ANTHROPIC_MAX_TOKENS", "anthropic.max_tokens")? {
        config = config.with_max_tokens(max_tokens)?;
    }
    Ok(Some(config))
}

#[cfg(feature = "agent-openai")]
fn load_openai_from_env() -> Result<Option<OpenAiConfig>, DomainError> {
    let Some(api_key_raw) = env_nonempty("MADE_OPENAI_API_KEY") else {
        return Ok(None);
    };
    let api_key = OpenAiApiKey::new(api_key_raw)?;
    let mut config = OpenAiConfig::new(api_key);
    if let Some(model) = env_nonempty("MADE_OPENAI_MODEL") {
        config = config.with_model(model)?;
    }
    if let Some(endpoint) = env_nonempty("MADE_OPENAI_ENDPOINT") {
        config = config.with_endpoint(endpoint)?;
    }
    if let Some(max_tokens) = env_u32("MADE_OPENAI_MAX_TOKENS", "openai.max_tokens")? {
        config = config.with_max_tokens(max_tokens)?;
    }
    Ok(Some(config))
}

#[cfg(feature = "agent-vllm")]
fn load_vllm_from_env() -> Result<Option<VllmConfig>, DomainError> {
    let Some(model) = env_nonempty("MADE_VLLM_MODEL") else {
        return Ok(None);
    };
    let Some(endpoint) = env_nonempty("MADE_VLLM_ENDPOINT") else {
        return Ok(None);
    };
    let mut config = VllmConfig::new(model)?.with_endpoint(endpoint)?;
    if let Some(bearer_raw) = env_nonempty("MADE_VLLM_BEARER_TOKEN") {
        config = config.with_bearer(VllmBearerToken::new(bearer_raw)?);
    }
    if let Some(max_tokens) = env_u32("MADE_VLLM_MAX_TOKENS", "vllm.max_tokens")? {
        config = config.with_max_tokens(max_tokens)?;
    }
    if let Some(timeout) = env_duration_secs("MADE_VLLM_TIMEOUT_SECS", "vllm.timeout_secs")? {
        config = config.with_timeout(timeout);
    }
    Ok(Some(config))
}

// ---------------------------------------------------------------------------
// Per-descriptor attribute overrides
// ---------------------------------------------------------------------------

#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
fn attr_string<'a>(
    attrs: &'a Attributes,
    key: &'static str,
) -> Result<Option<&'a str>, DomainError> {
    let Some(value) = attrs.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let text = value.as_str().ok_or(DomainError::InvariantViolated {
        reason: "agent factory: descriptor override must be a string",
    })?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
fn attr_u32(attrs: &Attributes, key: &'static str) -> Result<Option<u32>, DomainError> {
    let Some(value) = attrs.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let n = value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .or_else(|| {
            value.as_f64().and_then(|n| {
                if n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= f64::from(u32::MAX) {
                    Some(n as u32)
                } else {
                    None
                }
            })
        })
        .ok_or(DomainError::InvariantViolated {
            reason: "agent factory: provider numeric override must be a non-negative u32",
        })?;
    Ok(Some(n))
}

#[cfg(feature = "agent-anthropic")]
fn apply_anthropic_attributes(
    mut config: AnthropicConfig,
    attrs: &Attributes,
) -> Result<AnthropicConfig, DomainError> {
    if let Some(model) = attr_string(attrs, ATTR_MODEL)? {
        config = config.with_model(model)?;
    }
    if let Some(endpoint) = attr_string(attrs, ATTR_ENDPOINT)? {
        config = config.with_endpoint(endpoint)?;
    }
    if let Some(max_tokens) = attr_u32(attrs, ATTR_MAX_TOKENS)? {
        config = config.with_max_tokens(max_tokens)?;
    }
    Ok(config)
}

#[cfg(feature = "agent-openai")]
fn apply_openai_attributes(
    mut config: OpenAiConfig,
    attrs: &Attributes,
) -> Result<OpenAiConfig, DomainError> {
    if let Some(model) = attr_string(attrs, ATTR_MODEL)? {
        config = config.with_model(model)?;
    }
    if let Some(endpoint) = attr_string(attrs, ATTR_ENDPOINT)? {
        config = config.with_endpoint(endpoint)?;
    }
    if let Some(max_tokens) = attr_u32(attrs, ATTR_MAX_TOKENS)? {
        config = config.with_max_tokens(max_tokens)?;
    }
    Ok(config)
}

#[cfg(feature = "agent-vllm")]
fn apply_vllm_attributes(
    mut config: VllmConfig,
    attrs: &Attributes,
) -> Result<VllmConfig, DomainError> {
    if let Some(model) = attr_string(attrs, ATTR_MODEL)? {
        config = config.with_model(model)?;
    }
    if let Some(endpoint) = attr_string(attrs, ATTR_ENDPOINT)? {
        config = config.with_endpoint(endpoint)?;
    }
    if let Some(max_tokens) = attr_u32(attrs, ATTR_MAX_TOKENS)? {
        config = config.with_max_tokens(max_tokens)?;
    }
    if let Some(timeout_secs) = attr_u32(attrs, ATTR_TIMEOUT_SECS)? {
        if timeout_secs == 0 {
            return Err(DomainError::MustBeNonZero {
                field: "vllm.timeout_secs",
            });
        }
        config = config.with_timeout(Duration::from_secs(timeout_secs.into()));
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    use made_core::value_objects::{AgentId, AgentKind, Attributes, Specialty};
    #[cfg(any(feature = "agent-anthropic", feature = "agent-vllm"))]
    use serde_json::json;
    #[cfg(any(feature = "agent-anthropic", feature = "agent-vllm"))]
    use std::collections::BTreeMap;

    // Tests touch process-wide env vars. Serialize them with a local
    // mutex so cargo test's default parallel runner cannot interleave.
    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn descriptor(id: &str, kind: &str, attrs: Attributes) -> AgentDescriptor {
        AgentDescriptor {
            id: AgentId::new(id).unwrap(),
            specialty: Specialty::new("triage").unwrap(),
            kind: AgentKind::new(kind).unwrap(),
            attributes: attrs,
        }
    }

    fn clear_provider_env() {
        for key in [
            "MADE_ANTHROPIC_API_KEY",
            "MADE_ANTHROPIC_MODEL",
            "MADE_ANTHROPIC_ENDPOINT",
            "MADE_ANTHROPIC_MAX_TOKENS",
            "MADE_OPENAI_API_KEY",
            "MADE_OPENAI_MODEL",
            "MADE_OPENAI_ENDPOINT",
            "MADE_OPENAI_MAX_TOKENS",
            "MADE_VLLM_MODEL",
            "MADE_VLLM_ENDPOINT",
            "MADE_VLLM_BEARER_TOKEN",
            "MADE_VLLM_MAX_TOKENS",
            "MADE_VLLM_TIMEOUT_SECS",
        ] {
            std::env::remove_var(key);
        }
    }

    #[tokio::test]
    async fn noop_kind_is_always_materialized() {
        let factory = DispatchingAgentFactory::new();
        let agent = factory
            .create(descriptor("a1", "noop", Attributes::empty()))
            .await
            .unwrap();
        assert_eq!(agent.id().as_str(), "a1");
        assert_eq!(agent.specialty().as_str(), "triage");
    }

    #[tokio::test]
    async fn unknown_kind_is_rejected() {
        let factory = DispatchingAgentFactory::new();
        let err = factory
            .create(descriptor("a1", "alien", Attributes::empty()))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "agent factory: unsupported agent kind"
            }
        ));
    }

    #[tokio::test]
    async fn supported_kinds_lists_only_configured_providers() {
        let _guard = ENV_LOCK.lock().await;
        clear_provider_env();
        let factory = DispatchingAgentFactory::from_env().unwrap();
        assert_eq!(factory.supported_kinds(), vec!["noop"]);
    }

    #[cfg(feature = "agent-anthropic")]
    #[tokio::test]
    async fn anthropic_kind_requires_configured_base() {
        let factory = DispatchingAgentFactory::new();
        let err = factory
            .create(descriptor("a1", "anthropic", Attributes::empty()))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "agent factory: anthropic kind requested but not configured"
            }
        ));
    }

    #[cfg(feature = "agent-anthropic")]
    #[tokio::test]
    async fn anthropic_kind_with_configured_base_materializes() {
        let factory = DispatchingAgentFactory::new().with_anthropic(AnthropicConfig::new(
            AnthropicApiKey::new("sk-ant-test").unwrap(),
        ));
        let agent = factory
            .create(descriptor("a-anth", "anthropic", Attributes::empty()))
            .await
            .unwrap();
        assert_eq!(agent.id().as_str(), "a-anth");
        assert_eq!(agent.specialty().as_str(), "triage");
    }

    #[cfg(feature = "agent-anthropic")]
    #[tokio::test]
    async fn anthropic_kind_applies_descriptor_attribute_overrides() {
        let factory = DispatchingAgentFactory::new().with_anthropic(AnthropicConfig::new(
            AnthropicApiKey::new("sk-ant-test").unwrap(),
        ));
        let attrs = Attributes::new(BTreeMap::from([
            (ATTR_MODEL.to_owned(), json!("claude-3-7-sonnet")),
            (ATTR_MAX_TOKENS.to_owned(), json!(1024_u64)),
        ]))
        .unwrap();
        // Materialization succeeds; specific config values are private
        // to the adapter — verifying through the descriptor.attributes
        // path is enough to prove overrides are read without error.
        factory
            .create(descriptor("a-anth", "anthropic", attrs))
            .await
            .unwrap();
    }

    #[cfg(feature = "agent-anthropic")]
    #[tokio::test]
    async fn descriptor_with_wrong_typed_override_is_rejected() {
        let factory = DispatchingAgentFactory::new().with_anthropic(AnthropicConfig::new(
            AnthropicApiKey::new("sk-ant-test").unwrap(),
        ));
        let attrs = Attributes::new(BTreeMap::from([(
            ATTR_MODEL.to_owned(),
            json!(42), // model must be a string
        )]))
        .unwrap();
        let err = factory
            .create(descriptor("a-anth", "anthropic", attrs))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "agent factory: descriptor override must be a string"
            }
        ));
    }

    #[cfg(feature = "agent-anthropic")]
    #[tokio::test]
    async fn from_env_picks_up_anthropic_key() {
        let _guard = ENV_LOCK.lock().await;
        clear_provider_env();
        std::env::set_var("MADE_ANTHROPIC_API_KEY", "sk-ant-from-env");
        let factory = DispatchingAgentFactory::from_env().unwrap();
        assert!(factory.supported_kinds().contains(&"anthropic"));
        clear_provider_env();
    }

    #[cfg(feature = "agent-vllm")]
    #[tokio::test]
    async fn vllm_kind_requires_model_and_endpoint() {
        let _guard = ENV_LOCK.lock().await;
        clear_provider_env();
        std::env::set_var("MADE_VLLM_MODEL", "Qwen3.5-9B");
        // endpoint missing
        let factory = DispatchingAgentFactory::from_env().unwrap();
        assert_eq!(factory.supported_kinds(), vec!["noop"]);
        clear_provider_env();
    }

    #[cfg(feature = "agent-vllm")]
    #[tokio::test]
    async fn vllm_kind_picks_up_both_env_vars() {
        let _guard = ENV_LOCK.lock().await;
        clear_provider_env();
        std::env::set_var("MADE_VLLM_MODEL", "Qwen3.5-9B");
        std::env::set_var("MADE_VLLM_ENDPOINT", "https://vllm.example/v1");
        let factory = DispatchingAgentFactory::from_env().unwrap();
        assert!(factory.supported_kinds().contains(&"vllm"));
        clear_provider_env();
    }

    #[cfg(feature = "agent-vllm")]
    #[tokio::test]
    async fn vllm_kind_accepts_protobuf_numeric_overrides() {
        let factory = DispatchingAgentFactory::new().with_vllm(
            VllmConfig::new("Qwen3.5-9B")
                .unwrap()
                .with_endpoint("https://vllm.example")
                .unwrap(),
        );
        let attrs = Attributes::new(BTreeMap::from([
            (ATTR_MAX_TOKENS.to_owned(), json!(512.0)),
            (ATTR_TIMEOUT_SECS.to_owned(), json!(300.0)),
        ]))
        .unwrap();

        factory
            .create(descriptor("a-vllm", "vllm", attrs))
            .await
            .unwrap();
    }

    #[cfg(feature = "agent-vllm")]
    #[tokio::test]
    async fn vllm_kind_rejects_fractional_numeric_override() {
        let factory = DispatchingAgentFactory::new().with_vllm(
            VllmConfig::new("Qwen3.5-9B")
                .unwrap()
                .with_endpoint("https://vllm.example")
                .unwrap(),
        );
        let attrs =
            Attributes::new(BTreeMap::from([(ATTR_MAX_TOKENS.to_owned(), json!(512.5))])).unwrap();

        let err = factory
            .create(descriptor("a-vllm", "vllm", attrs))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "agent factory: provider numeric override must be a non-negative u32"
            }
        ));
    }

    #[cfg(feature = "agent-vllm")]
    #[tokio::test]
    async fn vllm_kind_rejects_invalid_timeout_env() {
        let _guard = ENV_LOCK.lock().await;
        clear_provider_env();
        std::env::set_var("MADE_VLLM_MODEL", "Qwen3.5-9B");
        std::env::set_var("MADE_VLLM_ENDPOINT", "https://vllm.example/v1");
        std::env::set_var("MADE_VLLM_TIMEOUT_SECS", "slow");
        let err = DispatchingAgentFactory::from_env().unwrap_err();
        assert!(matches!(
            err,
            DomainError::InvariantViolated {
                reason: "vllm.timeout_secs"
            }
        ));
        clear_provider_env();
    }
}
