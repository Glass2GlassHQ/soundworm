//! WASAPI backend for soundworm (stub).
//!
//! Real Windows bindings land in v0.6. This stub exists so the daemon's
//! cfg-gated backend selection compiles on Windows targets — every
//! method returns `SoundwormError::Backend("wasapi unavailable")` so
//! running `swd` on Windows fails loudly rather than silently.

use async_trait::async_trait;
use soundworm_core::{
    backend::AudioBackend,
    error::{Result, SoundwormError},
    event::BackendEvent,
    link::Link,
    node::Node,
};
use std::sync::mpsc;

pub struct WasapiBackend;

impl WasapiBackend {
    pub fn new() -> Result<Self> { Ok(Self) }
}

#[async_trait]
impl AudioBackend for WasapiBackend {
    fn name(&self) -> &str { "wasapi" }
    fn subscribe(&self) -> mpsc::Receiver<BackendEvent> { mpsc::channel().1 }
    async fn enumerate_nodes(&self) -> Result<Vec<Node>> {
        Err(SoundwormError::Backend("wasapi backend not yet implemented (v0.6)".into()))
    }
    async fn create_link(&self, _l: &Link) -> Result<()> {
        Err(SoundwormError::Backend("wasapi backend not yet implemented (v0.6)".into()))
    }
    async fn destroy_link(&self, _l: &Link) -> Result<()> { Ok(()) }
    async fn set_volume(&self, _n: u64, _v: f32) -> Result<()> {
        Err(SoundwormError::Backend("wasapi backend not yet implemented (v0.6)".into()))
    }
}
