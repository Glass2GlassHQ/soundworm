use async_trait::async_trait;
use soundworm_core::{
    backend::AudioBackend,
    error::Result,
    event::BackendEvent,
    link::Link,
    node::Node,
};
#[cfg(not(target_os = "macos"))]
use soundworm_core::error::SoundwormError;
use std::sync::mpsc;

/// CoreAudio backend handle. Cheap to clone-not-implemented; the
/// daemon owns it behind an `Arc<dyn AudioBackend>`.
///
/// On macOS this wraps the running HAL listener thread (see
/// [`crate::macos`]). On other targets it's an inert stub — every
/// method returns an error so that accidentally running `swd` against
/// this backend on Linux/Windows fails loudly rather than silently.
pub struct CoreAudioBackend {
    #[cfg(target_os = "macos")]
    inner: crate::macos::Inner,
}

impl CoreAudioBackend {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            Ok(Self { inner: crate::macos::Inner::start()? })
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(SoundwormError::Backend(
                "CoreAudio backend selected on a non-macOS target".into(),
            ))
        }
    }
}

#[async_trait]
impl AudioBackend for CoreAudioBackend {
    fn name(&self) -> &str { "coreaudio" }

    fn subscribe(&self) -> mpsc::Receiver<BackendEvent> {
        #[cfg(target_os = "macos")]
        {
            self.inner.subscribe()
        }
        #[cfg(not(target_os = "macos"))]
        {
            mpsc::channel().1
        }
    }

    async fn enumerate_nodes(&self) -> Result<Vec<Node>> {
        #[cfg(target_os = "macos")]
        {
            self.inner.enumerate_nodes()
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(SoundwormError::Backend("coreaudio unavailable on this target".into()))
        }
    }

    async fn create_link(&self, link: &Link) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.inner.set_default_output(link.sink_port.0)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = link;
            Err(SoundwormError::Backend("coreaudio unavailable on this target".into()))
        }
    }

    async fn destroy_link(&self, _link: &Link) -> Result<()> {
        // CoreAudio has no "unlink" — see crate-level docs. We accept
        // the call so the daemon's bookkeeping stays simple and log
        // a tracing event so operators see what happened.
        tracing::debug!("coreaudio destroy_link is a no-op (HAL has no port-to-port unlinking)");
        Ok(())
    }

    async fn set_volume(&self, node_id: u64, volume: f32) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.inner.set_volume(node_id, volume)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (node_id, volume);
            Err(SoundwormError::Backend("coreaudio unavailable on this target".into()))
        }
    }

    async fn set_mute(&self, node_id: u64, mute: bool) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.inner.set_mute(node_id, mute)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (node_id, mute);
            Err(SoundwormError::Backend("coreaudio unavailable on this target".into()))
        }
    }
}
