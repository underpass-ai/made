/// The five RPCs of the integrator loop.
///
/// Their own link in the chain rather than more of the ceremony one,
/// because they are authorized by a scope that is a message and not a
/// field: three of them name a ceremony *or* a system run, one names a
/// binding and nothing else, and the read names either or neither.
macro_rules! integrator_loop_rpc_methods {
    ($callback:ident; { $($methods:tt)* }) => {
        resource_rpc_methods!($callback; {
            $($methods)*
        async fn bind_ceremony_integrator(
            &self,
            request: Request<pb::BindCeremonyIntegratorRequest>,
        ) -> GrpcResult<pb::BindCeremonyIntegratorResponse> {
            authorized_integrator_scope!(
                self,
                request,
                BindCeremonyIntegrator,
                self.handle_bind_ceremony_integrator(request)
            )
        }
        async fn get_ceremony_integrator_binding(
            &self,
            request: Request<pb::GetCeremonyIntegratorBindingRequest>,
        ) -> GrpcResult<pb::GetCeremonyIntegratorBindingResponse> {
            authorized_integrator_scope!(
                self,
                request,
                GetCeremonyIntegratorBinding,
                self.handle_get_ceremony_integrator_binding(request)
            )
        }
        async fn await_integrator_attention(
            &self,
            request: Request<pb::AwaitIntegratorAttentionRequest>,
        ) -> GrpcResult<pb::AwaitIntegratorAttentionResponse> {
            authorized_integrator_scope!(
                self,
                request,
                AwaitIntegratorAttention,
                self.handle_await_integrator_attention(request)
            )
        }
        // Global: an acknowledgement names a binding and a lease, and
        // resolving that binding to a ceremony here would authorize
        // against whatever the ledger says rather than against what the
        // caller asked for.
        async fn acknowledge_integrator_attention(
            &self,
            request: Request<pb::AcknowledgeIntegratorAttentionRequest>,
        ) -> GrpcResult<pb::AcknowledgeIntegratorAttentionResponse> {
            authorized_global!(
                self,
                request,
                AcknowledgeIntegratorAttention,
                self.handle_acknowledge_integrator_attention(request)
            )
        }
        async fn list_attention_deliveries(
            &self,
            request: Request<pb::ListAttentionDeliveriesRequest>,
        ) -> GrpcResult<pb::ListAttentionDeliveriesResponse> {
            authorized_ceremony_or_global!(
                self,
                request,
                ListAttentionDeliveries,
                self.handle_list_attention_deliveries(request)
            )
        }
        });
    };
}

pub(super) use integrator_loop_rpc_methods;
