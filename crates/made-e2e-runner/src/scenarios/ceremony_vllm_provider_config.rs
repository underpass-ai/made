use anyhow::{anyhow, bail, Context, Result};

const VLLM_ENDPOINT_ENV: &str = "MADE_VLLM_ENDPOINT";
const VLLM_MODEL_ENV: &str = "MADE_VLLM_MODEL";
const VLLM_MAX_TOKENS_ENV: &str = "MADE_VLLM_MAX_TOKENS";
const VLLM_TIMEOUT_SECS_ENV: &str = "MADE_VLLM_TIMEOUT_SECS";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CeremonyVllmProviderConfig {
    endpoint: String,
    model: String,
    max_tokens: u32,
    timeout_secs: u32,
}

impl CeremonyVllmProviderConfig {
    const DEFAULT_MAX_TOKENS: u32 = 512;
    const DEFAULT_TIMEOUT_SECS: u32 = 300;

    pub(crate) fn from_env() -> Result<Self> {
        Self::new(
            required_env(VLLM_ENDPOINT_ENV)?,
            required_env(VLLM_MODEL_ENV)?,
            env_u32(VLLM_MAX_TOKENS_ENV, Self::DEFAULT_MAX_TOKENS)?,
            env_u32(VLLM_TIMEOUT_SECS_ENV, Self::DEFAULT_TIMEOUT_SECS)?,
        )
    }

    pub(crate) fn new(
        endpoint: impl Into<String>,
        model: impl Into<String>,
        max_tokens: u32,
        timeout_secs: u32,
    ) -> Result<Self> {
        let endpoint = trimmed_nonempty(endpoint.into(), "vllm.endpoint")?;
        let model = trimmed_nonempty(model.into(), "vllm.model")?;
        if max_tokens == 0 {
            bail!("vllm.max_tokens must be greater than zero");
        }
        if timeout_secs == 0 {
            bail!("vllm.timeout_secs must be greater than zero");
        }
        Ok(Self {
            endpoint,
            model,
            max_tokens,
            timeout_secs,
        })
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }

    pub(crate) fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    pub(crate) fn timeout_secs(&self) -> u32 {
        self.timeout_secs
    }
}

fn required_env(name: &str) -> Result<String> {
    let raw = std::env::var(name).map_err(|_| anyhow!("required env var {name} is not set"))?;
    trimmed_nonempty(raw, name)
}

fn env_u32(name: &str, default: u32) -> Result<u32> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|raw| {
            raw.parse::<u32>()
                .with_context(|| format!("{name} must parse as u32"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn trimmed_nonempty(raw: impl Into<String>, field: &str) -> Result<String> {
    let trimmed = raw.into().trim().to_owned();
    if trimmed.is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_provider_values() {
        let config = CeremonyVllmProviderConfig::new(
            " http://stub-llm:8000 ",
            " stub-report-vllm-v1 ",
            128,
            30,
        )
        .unwrap();

        assert_eq!(config.endpoint(), "http://stub-llm:8000");
        assert_eq!(config.model(), "stub-report-vllm-v1");
        assert_eq!(config.max_tokens(), 128);
        assert_eq!(config.timeout_secs(), 30);
    }

    #[test]
    fn rejects_zero_bounds() {
        assert!(CeremonyVllmProviderConfig::new("http://stub-llm:8000", "model", 0, 30).is_err());
        assert!(CeremonyVllmProviderConfig::new("http://stub-llm:8000", "model", 128, 0).is_err());
    }
}
