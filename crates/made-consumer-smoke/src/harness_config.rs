use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HarnessConfig {
    pub endpoint: String,
    pub nats_url: Option<String>,
    pub specialty: String,
    pub contract_id: String,
    pub connect_budget: Duration,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:50055".to_owned(),
            nats_url: None,
            specialty: "triage".to_owned(),
            contract_id: "consumer-smoke-report-v1".to_owned(),
            connect_budget: Duration::from_secs(30),
        }
    }
}
