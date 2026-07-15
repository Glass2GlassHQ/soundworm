//! WASAPI backend for soundworm.
//!
//! On Windows this enumerates audio endpoints via the MMDevice API (see
//! [`win`]). Routing is a documented gap: Windows has no public API to
//! set the default endpoint (it needs the undocumented IPolicyConfig),
//! so `create_link` errors rather than pretending. On other targets the
//! whole backend is an inert stub that fails loudly.

use async_trait::async_trait;
use soundworm_core::{
    backend::AudioBackend,
    error::{Result, SoundwormError},
    event::BackendEvent,
    link::Link,
    node::Node,
};
use std::sync::mpsc;

#[cfg(target_os = "windows")]
mod win;

pub struct WasapiBackend;

impl WasapiBackend {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl AudioBackend for WasapiBackend {
    fn name(&self) -> &str {
        "wasapi"
    }

    fn subscribe(&self) -> mpsc::Receiver<BackendEvent> {
        // TODO(v0.6): IMMNotificationClient → NodeAppeared/NodeRemoved.
        mpsc::channel().1
    }

    async fn enumerate_nodes(&self) -> Result<Vec<Node>> {
        #[cfg(target_os = "windows")]
        {
            Ok(win::enumerate_endpoints())
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(SoundwormError::Backend("wasapi unavailable on this target".into()))
        }
    }

    async fn create_link(&self, _link: &Link) -> Result<()> {
        Err(SoundwormError::Backend(
            "wasapi routing unsupported: no public API to set the default endpoint".into(),
        ))
    }

    async fn destroy_link(&self, _link: &Link) -> Result<()> {
        Ok(())
    }

    async fn set_volume(&self, _node_id: u64, _volume: f32) -> Result<()> {
        Err(SoundwormError::Backend("wasapi set_volume not yet implemented".into()))
    }
}
