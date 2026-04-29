#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

#[cfg(not(target_arch = "wasm32"))]
mod unsupported {
    use anyhow::{anyhow, Result};
    use openim_protocol::{GeneralWsReq, GeneralWsResp};
    use openim_transport_core::{TransportConfig, TransportEvent};

    pub type ClientConfig = TransportConfig;

    pub struct WasmWsClient;

    impl WasmWsClient {
        pub async fn connect(_config: TransportConfig) -> Result<Self> {
            Err(anyhow!(
                "openim-transport-wasm is only available on wasm32 targets"
            ))
        }

        pub fn config(&self) -> &TransportConfig {
            unreachable!("wasm transport is unsupported on this target")
        }

        pub async fn send_request(&mut self, _req: &GeneralWsReq) -> Result<()> {
            Err(anyhow!(
                "openim-transport-wasm is only available on wasm32 targets"
            ))
        }

        pub async fn send_heartbeat_ping(&mut self) -> Result<()> {
            Err(anyhow!(
                "openim-transport-wasm is only available on wasm32 targets"
            ))
        }

        pub async fn recv_event(&mut self) -> Result<TransportEvent> {
            Err(anyhow!(
                "openim-transport-wasm is only available on wasm32 targets"
            ))
        }

        pub async fn recv_envelope(&mut self) -> Result<GeneralWsResp> {
            Err(anyhow!(
                "openim-transport-wasm is only available on wasm32 targets"
            ))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use unsupported::*;
