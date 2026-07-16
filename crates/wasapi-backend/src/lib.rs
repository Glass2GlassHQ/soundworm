//! WASAPI backend for soundworm.
//!
//! On Windows this drives the MMDevice API (see [`win`]): it enumerates
//! render/capture endpoints, routes by setting the default endpoint,
//! sets per-endpoint volume, and emits live device notifications.
//! Windows has no port-to-port linking, so a link means "make this the
//! default endpoint" (the coreaudio backend uses the same model); the
//! default is set via the undocumented `IPolicyConfig` interface (see
//! [`win`] for the caveat). On other targets the whole backend is an
//! inert stub that fails loudly.

use async_trait::async_trait;
use soundworm_core::{
    backend::AudioBackend,
    error::Result,
    event::BackendEvent,
    link::Link,
    node::Node,
};
#[cfg(not(target_os = "windows"))]
use soundworm_core::error::SoundwormError;
use std::sync::mpsc;

#[cfg(target_os = "windows")]
mod win;

pub struct WasapiBackend {
    #[cfg(target_os = "windows")]
    inner: win::Inner,
}

impl WasapiBackend {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "windows")]
        {
            Ok(Self { inner: win::Inner::start()? })
        }
        #[cfg(not(target_os = "windows"))]
        {
            Ok(Self {})
        }
    }
}

#[async_trait]
impl AudioBackend for WasapiBackend {
    fn name(&self) -> &str {
        "wasapi"
    }

    fn subscribe(&self) -> mpsc::Receiver<BackendEvent> {
        #[cfg(target_os = "windows")]
        {
            self.inner.subscribe()
        }
        #[cfg(not(target_os = "windows"))]
        {
            mpsc::channel().1
        }
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

    async fn create_link(&self, link: &Link) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            // A link routes to its sink: set that endpoint as the default.
            // Port ids equal node ids for endpoint-default backends.
            win::set_default_endpoint(link.sink_port.0)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = link;
            Err(SoundwormError::Backend("wasapi unavailable on this target".into()))
        }
    }

    async fn destroy_link(&self, _link: &Link) -> Result<()> {
        // Windows has no "unlink"; you can't un-default an endpoint.
        tracing::debug!("wasapi destroy_link is a no-op (no port-to-port unlinking)");
        Ok(())
    }

    async fn set_volume(&self, node_id: u64, volume: f32) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            win::set_endpoint_volume(node_id, volume)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (node_id, volume);
            Err(SoundwormError::Backend("wasapi unavailable on this target".into()))
        }
    }
}
