/// Stable finish reasons suitable for evidence and metrics dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCall,
    ContentFilter,
    NotReported,
}
