//! `tonic::Status` → [`ToolError`], in one place.
//!
//! The transport's own vocabulary stops here. A client reading a tool
//! result must not have to know that this backend spoke gRPC, which is
//! what `"gRPC {code}: {message}"` made it do.

use tonic::{Code, Status};

use crate::protocol::ToolError;

impl From<Status> for ToolError {
    fn from(status: Status) -> Self {
        let message = status.message().to_owned();
        match status.code() {
            Code::NotFound => Self::not_found(message),
            // Both mean the engine was not reached, not that it looked
            // and said no; waiting is the remedy for either.
            Code::Unavailable | Code::DeadlineExceeded => Self::unavailable(message),
            Code::InvalidArgument => Self::invalid_request(message),
            // The server answers `aborted` for one thing only:
            // `DomainError::Conflict`, a write that lost a race. Its
            // remedy is to read the session again and repeat the call,
            // which is the opposite of what `refused` tells a client.
            Code::Aborted => Self::conflict(message),
            // Everything else — failed preconditions, already-exists,
            // internal — is the engine answering. The remedy is to
            // change the call or the session, never to repeat it
            // unchanged.
            _ => Self::refused(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ToolErrorCode;

    #[test]
    fn every_status_lands_on_one_of_the_five_codes() {
        let cases = [
            (Status::not_found("nope"), ToolErrorCode::NotFound),
            (Status::unavailable("down"), ToolErrorCode::Unavailable),
            (
                Status::deadline_exceeded("slow"),
                ToolErrorCode::Unavailable,
            ),
            (
                Status::invalid_argument("bad"),
                ToolErrorCode::InvalidRequest,
            ),
            (Status::failed_precondition("no"), ToolErrorCode::Refused),
            (Status::aborted("raced"), ToolErrorCode::Conflict),
            (Status::already_exists("twice"), ToolErrorCode::Refused),
            (Status::internal("boom"), ToolErrorCode::Refused),
        ];
        for (status, expected) in cases {
            let code = status.code();
            assert_eq!(
                ToolError::from(status).code(),
                expected,
                "{code} should map to {expected}"
            );
        }
    }

    #[test]
    fn the_transport_never_reaches_the_message() {
        let error = ToolError::from(Status::not_found("no ceremony named `c-1`"));
        assert_eq!(error.message(), "no ceremony named `c-1`");
    }
}
