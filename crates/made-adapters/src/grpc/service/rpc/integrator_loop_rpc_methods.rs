/// The five RPCs of the integrator loop.
///
/// Their own link in the chain rather than more of the protocol one,
/// because they are authorized by a scope that is a message and not a
/// field: three of them name a protocol *or* a system run, one names a
/// binding and nothing else, and the read names either or neither.
macro_rules! integrator_loop_rpc_methods {
    ($callback:ident; { $($methods:tt)* }) => {
        resource_rpc_methods!($callback; {
            $($methods)*
        async fn bind_protocol_integrator(
            &self,
            request: Request<pb::BindProtocolIntegratorRequest>,
        ) -> GrpcResult<pb::BindProtocolIntegratorResponse> {
            authorized_integrator_scope!(
                self,
                request,
                BindProtocolIntegrator,
                self.handle_bind_protocol_integrator(request)
            )
        }
        async fn get_protocol_integrator_binding(
            &self,
            request: Request<pb::GetProtocolIntegratorBindingRequest>,
        ) -> GrpcResult<pb::GetProtocolIntegratorBindingResponse> {
            authorized_integrator_scope!(
                self,
                request,
                GetProtocolIntegratorBinding,
                self.handle_get_protocol_integrator_binding(request)
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
        // resolving that binding to a protocol here would authorize
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
            authorized_protocol_or_global!(
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
