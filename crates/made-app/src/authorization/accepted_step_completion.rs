use made_core::value_objects::{
    AuthenticatedPrincipal, AuthorizationRequestId, AuthorizationTargetDigest, CeremonyId,
    StepClaimFence, StepId,
};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedStepCompletion {
    pub ceremony_id: CeremonyId,
    pub step_id: StepId,
    pub claim_fence: StepClaimFence,
    pub principal: AuthenticatedPrincipal,
    pub request_id: AuthorizationRequestId,
    pub target_digest: AuthorizationTargetDigest,
}

impl AcceptedStepCompletion {
    pub fn from_invocation(
        ceremony_id: CeremonyId,
        step_id: StepId,
        claim_fence: StepClaimFence,
        principal: AuthenticatedPrincipal,
        invocation_id: &AuthorizationRequestId,
        target_digest: AuthorizationTargetDigest,
    ) -> Result<Self, made_core::DomainError> {
        let mut digest = Sha256::new();
        digest.update(b"made.accepted-step-completion.v1\0");
        for part in [invocation_id.as_str(), claim_fence.as_str()] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part.as_bytes());
        }
        Ok(Self {
            ceremony_id,
            step_id,
            claim_fence,
            principal,
            request_id: AuthorizationRequestId::new(format!(
                "accepted-step:{:x}",
                digest.finalize()
            ))?,
            target_digest,
        })
    }
}
