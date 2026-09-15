//! Synchronous owner of protocol dispatch state.
//!
//! Transports depend on [`ProtocolDispatch`], not on Repository or storage
//! internals. This keeps one Core-owned Repository behind every local or remote
//! protocol entry point.

use crate::protocol::{
    ExternalAgentProtocol, ProtocolRequest, ProtocolResponse, WorkspaceBindingResolver,
};
use crate::Repository;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolDispatchError;

pub trait ProtocolDispatch: Send + Sync + 'static {
    fn dispatch(
        &self,
        request: ProtocolRequest,
        now_ms: i64,
    ) -> Result<ProtocolResponse, ProtocolDispatchError>;
}

pub struct AgentProtocolCore<B> {
    repository: Mutex<Repository>,
    bindings: B,
}

impl<B> AgentProtocolCore<B> {
    pub fn new(repository: Repository, bindings: B) -> Self {
        Self {
            repository: Mutex::new(repository),
            bindings,
        }
    }
}

impl<B> ProtocolDispatch for AgentProtocolCore<B>
where
    B: WorkspaceBindingResolver + Send + Sync + 'static,
{
    fn dispatch(
        &self,
        request: ProtocolRequest,
        now_ms: i64,
    ) -> Result<ProtocolResponse, ProtocolDispatchError> {
        let mut repository = self.repository.lock().map_err(|_| ProtocolDispatchError)?;
        Ok(ExternalAgentProtocol::new(&mut repository, &self.bindings).handle(request, now_ms))
    }
}
