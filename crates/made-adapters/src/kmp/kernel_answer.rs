use serde_json::Value;

/// What a kernel tool call came back with.
///
/// A tool that refuses is not a broken pipe. "There is no memory
/// under that name" is an answer, and one this adapter reads as an
/// empty scope rather than a failure; a transport that flattened the
/// two would make an unreachable kernel indistinguishable from a
/// session nobody has written about yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelAnswer {
    /// The tool ran and returned its document.
    Returned(Value),
    /// The tool ran and refused, in the kernel's own words.
    Refused(String),
}
