/// Per-provider static error reasons.
///
/// Every adapter owns a `const` instance of this table and threads
/// it through the shared response helpers.
pub(in crate::agents) struct ErrorStrings {
    pub unauthorized: &'static str,
    pub rate_limited: &'static str,
    pub bad_request: &'static str,
    pub upstream_error: &'static str,
    pub malformed_body: &'static str,
    pub no_choices: &'static str,
    pub missing_content: &'static str,
    pub empty_content: &'static str,
}
