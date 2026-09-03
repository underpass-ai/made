/// Operator-visible TLS posture options for the Runtime client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeClientTlsMode {
    Disabled,
    Server,
    Mutual,
}
