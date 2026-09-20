use made_proto::v1::{
    ApproveCeremonyGuardRequest, CancelCeremonyRequest, CeremonyInstanceState,
    EnforceCeremonyDeadlinesRequest, PauseCeremonyRequest, ResumeCeremonyRequest,
};

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    pub async fn renew_step_lease(
        &self,
        request: made_proto::v1::RenewCeremonyStepLeaseRequest,
    ) -> Result<made_proto::v1::RenewCeremonyStepLeaseResponse, MadeClientError> {
        Ok(self
            .rpc()
            .renew_ceremony_step_lease(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/RenewCeremonyStepLease",
                request,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner())
    }

    pub async fn record_ceremony_host_handoff(
        &self,
        request: made_proto::v1::RecordCeremonyHostHandoffRequest,
    ) -> Result<made_proto::v1::RecordCeremonyHostHandoffResponse, MadeClientError> {
        self.rpc()
            .record_ceremony_host_handoff(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/RecordCeremonyHostHandoff",
                request,
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }
    pub async fn inspect_ceremony_resume(
        &self,
        request: made_proto::v1::InspectCeremonyResumeRequest,
    ) -> Result<made_proto::v1::InspectCeremonyResumeResponse, MadeClientError> {
        self.rpc()
            .inspect_ceremony_resume(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/InspectCeremonyResume",
                request,
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }
    pub async fn pause(
        &self,
        request: PauseCeremonyRequest,
    ) -> Result<CeremonyInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .pause_ceremony(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/PauseCeremony",
                request,
            ))
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
            .resume_ceremony(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ResumeCeremony",
                request,
            ))
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
            .cancel_ceremony(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/CancelCeremony",
                request,
            ))
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
            .enforce_ceremony_deadlines(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/EnforceCeremonyDeadlines",
                EnforceCeremonyDeadlinesRequest {
                    ceremony_id: ceremony_id.into(),
                },
            ))
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
            .approve_ceremony_guard(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ApproveCeremonyGuard",
                request,
            ))
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
