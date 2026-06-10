//! Agent provider adapters.
//!
//! Each provider is a peer adapter behind its own Cargo feature flag.
//! The Choreographer core is **provider-agnostic**: there is no
//! privileged vendor. Adding a new provider is always purely additive
//! — a new feature + a new module + a new `impl AgentPort`, no core
//! changes required.
//!
//! Secrets (API keys, bearer tokens, …) must never be printed through
//! `Debug` impls. Each provider adapter is expected to wrap its
//! credentials in an opaque type that masks the value on formatting.

// Shared infrastructure for provider adapters. `prompts` is reused
// by every provider that speaks natural language (so all current
// adapters); `openai_compat` is reused only by adapters that speak
// the Chat Completions wire shape (OpenAI + vLLM, not Anthropic).
//
// Both are `pub(super)` (i.e. visible only within `agents::*`).

#[cfg(any(
    feature = "agent-anthropic",
    feature = "agent-openai",
    feature = "agent-vllm"
))]
mod prompts;

#[cfg(any(feature = "agent-openai", feature = "agent-vllm"))]
mod openai_compat;

// Shared provider-endpoint validation (scheme allowlist, fail-fast).
// Available whenever any HTTP provider adapter is compiled in.
#[cfg(feature = "_http")]
mod endpoint;

// Shared latency + in-flight instrumentation for HTTP provider calls.
#[cfg(feature = "_http")]
mod instrument;

#[cfg(any(feature = "agent-openai", feature = "agent-vllm"))]
pub mod judge;

#[cfg(feature = "agent-anthropic")]
pub mod anthropic;

#[cfg(feature = "agent-openai")]
pub mod openai;

#[cfg(feature = "agent-vllm")]
pub mod vllm;

pub mod factory;
pub use factory::{
    DispatchingAgentFactory, ANTHROPIC_AGENT_KIND, OPENAI_AGENT_KIND, VLLM_AGENT_KIND,
};

use std::sync::Arc;

use choreo_core::error::DomainError;
use choreo_core::ports::{MetricsRecorderPort, ValidatorPort};

/// Build the LLM-judge validator from the environment, when enabled.
///
/// The judge is opt-in via `CHOREO_JUDGE_ENABLED` (`1`/`true`/`yes`). When
/// enabled it reuses the vLLM endpoint and model (`CHOREO_VLLM_ENDPOINT`,
/// `CHOREO_VLLM_MODEL`) and an optional `CHOREO_JUDGE_THRESHOLD` (default
/// `0.5`). The feature-gating lives here, mirroring
/// [`DispatchingAgentFactory::from_env`]: the composition root calls this
/// unconditionally and stays free of provider `cfg`s.
///
/// Returns `Ok(None)` when the judge is disabled, or when the binary was
/// built without a Chat-Completions provider feature (so the judge code is
/// not compiled in). Returns `Err` — failing fast — when the judge is
/// explicitly enabled but its endpoint/model/threshold are missing or
/// invalid.
pub fn judge_from_env(
    metrics: Arc<dyn MetricsRecorderPort>,
) -> Result<Option<Arc<dyn ValidatorPort>>, DomainError> {
    if !judge_enabled() {
        return Ok(None);
    }
    #[cfg(any(feature = "agent-openai", feature = "agent-vllm"))]
    {
        build_env_judge(metrics).map(Some)
    }
    #[cfg(not(any(feature = "agent-openai", feature = "agent-vllm")))]
    {
        let _ = metrics;
        tracing::warn!(
            "CHOREO_JUDGE_ENABLED is set but this build has no Chat-Completions \
             provider feature; scoring stays uniform"
        );
        Ok(None)
    }
}

/// Whether the operator opted the judge in via `CHOREO_JUDGE_ENABLED`.
fn judge_enabled() -> bool {
    std::env::var("CHOREO_JUDGE_ENABLED")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

/// Construct the judge from the vLLM endpoint/model env, failing fast on a
/// missing or invalid setting.
#[cfg(any(feature = "agent-openai", feature = "agent-vllm"))]
fn build_env_judge(
    metrics: Arc<dyn MetricsRecorderPort>,
) -> Result<Arc<dyn ValidatorPort>, DomainError> {
    let endpoint = std::env::var("CHOREO_VLLM_ENDPOINT").map_err(|_| DomainError::EmptyField {
        field: "judge.endpoint",
    })?;
    let model = std::env::var("CHOREO_VLLM_MODEL").map_err(|_| DomainError::EmptyField {
        field: "judge.model",
    })?;
    let threshold = std::env::var("CHOREO_JUDGE_THRESHOLD")
        .ok()
        .map_or(Ok(0.5), |value| {
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| DomainError::EmptyField {
                    field: "judge.threshold",
                })
        })?;
    let judge = judge::LlmJudgeValidator::new(endpoint, model, threshold, metrics)?;
    Ok(Arc::new(judge))
}
