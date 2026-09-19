use made_proto::v1::{
    ApproveCeremonyGuardRequest, CancelCeremonyRequest, CeremonyInstanceState,
    EnforceCeremonyDeadlinesRequest, PauseCeremonyRequest, ResumeCeremonyRequest,
};

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    pub async fn pause(
        &self,
        request: PauseCeremonyRequest,
    ) -> Result<CeremonyInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .pause_ceremony(self.request(request))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "pause")
    }

    pub async fn resume(
        &self,
        request: ResumeCeremonyRequest,
    ) -> Result<CeremonyInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .resume_ceremony(self.request(request))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "resume")
    }

    pub async fn cancel(
        &self,
        request: CancelCeremonyRequest,
    ) -> Result<CeremonyInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .cancel_ceremony(self.request(request))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "cancel")
    }

    pub async fn enforce_deadlines(
        &self,
        ceremony_id: impl Into<String>,
    ) -> Result<CeremonyInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .enforce_ceremony_deadlines(self.request(EnforceCeremonyDeadlinesRequest {
                ceremony_id: ceremony_id.into(),
            }))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "enforce deadlines")
    }

    pub async fn approve(
        &self,
        request: ApproveCeremonyGuardRequest,
    ) -> Result<CeremonyInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .approve_ceremony_guard(self.request(request))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "approve guard")
    }
}

fn required_instance(
    instance: Option<CeremonyInstanceState>,
    operation: &str,
) -> Result<CeremonyInstanceState, MadeClientError> {
    instance.ok_or_else(|| {
        MadeClientError::ProtocolViolation(format!("{operation} response has no instance"))
    })
}
