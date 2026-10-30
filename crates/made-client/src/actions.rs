use made_proto::v1::{
    ApproveProtocolGuardRequest, CancelProtocolRequest, EnforceProtocolDeadlinesRequest,
    PauseProtocolRequest, ProtocolInstanceState, ResumeProtocolRequest,
};

use crate::{MadeClient, MadeClientError};

impl MadeClient {
    pub async fn renew_step_lease(
        &self,
        request: made_proto::v1::RenewProtocolStepLeaseRequest,
    ) -> Result<made_proto::v1::RenewProtocolStepLeaseResponse, MadeClientError> {
        Ok(self
            .rpc()
            .renew_protocol_step_lease(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/RenewProtocolStepLease",
                request,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner())
    }

    pub async fn record_protocol_host_handoff(
        &self,
        request: made_proto::v1::RecordProtocolHostHandoffRequest,
    ) -> Result<made_proto::v1::RecordProtocolHostHandoffResponse, MadeClientError> {
        self.rpc()
            .record_protocol_host_handoff(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/RecordProtocolHostHandoff",
                request,
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }
    pub async fn inspect_protocol_resume(
        &self,
        request: made_proto::v1::InspectProtocolResumeRequest,
    ) -> Result<made_proto::v1::InspectProtocolResumeResponse, MadeClientError> {
        self.rpc()
            .inspect_protocol_resume(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/InspectProtocolResume",
                request,
            ))
            .await
            .map(tonic::Response::into_inner)
            .map_err(MadeClientError::from_status)
    }
    pub async fn pause(
        &self,
        request: PauseProtocolRequest,
    ) -> Result<ProtocolInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .pause_protocol(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/PauseProtocol",
                request,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "pause")
    }

    pub async fn resume(
        &self,
        request: ResumeProtocolRequest,
    ) -> Result<ProtocolInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .resume_protocol(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ResumeProtocol",
                request,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "resume")
    }

    pub async fn cancel(
        &self,
        request: CancelProtocolRequest,
    ) -> Result<ProtocolInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .cancel_protocol(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/CancelProtocol",
                request,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "cancel")
    }

    pub async fn enforce_deadlines(
        &self,
        protocol_id: impl Into<String>,
    ) -> Result<ProtocolInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .enforce_protocol_deadlines(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/EnforceProtocolDeadlines",
                EnforceProtocolDeadlinesRequest {
                    protocol_id: protocol_id.into(),
                },
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "enforce deadlines")
    }

    pub async fn approve(
        &self,
        request: ApproveProtocolGuardRequest,
    ) -> Result<ProtocolInstanceState, MadeClientError> {
        let response = self
            .rpc()
            .approve_protocol_guard(Self::request(
                &self.context(),
                "/underpass.made.v1.MadeService/ApproveProtocolGuard",
                request,
            ))
            .await
            .map_err(MadeClientError::from_status)?
            .into_inner();
        required_instance(response.instance, "approve guard")
    }
}

fn required_instance(
    instance: Option<ProtocolInstanceState>,
    operation: &str,
) -> Result<ProtocolInstanceState, MadeClientError> {
    instance.ok_or_else(|| {
        MadeClientError::ProtocolViolation(format!("{operation} response has no instance"))
    })
}
