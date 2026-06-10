//! macOS HAL bindings via `coreaudio-sys`.
//!
//! This module is the only part of the crate compiled on macOS targets.
//! The actual HAL calls are deliberately left as `TODO(v0.5-mac)`
//! stubs returning `Ok(empty)` / no-op — they need to be filled in on a
//! macOS dev box where the resulting code can actually be exercised.
//!
//! The structural pieces are real and ready:
//!   * `Inner::start()` brings up the HAL listener thread + event
//!     broadcaster, mirroring the `pipewire-backend` shape.
//!   * `device_id_to_node_id` / `node_id_to_device_id` settle the
//!     `AudioDeviceID` (u32) ↔ `NodeId` (u64) mapping.
//!   * `hal_devices()` is where `kAudioHardwarePropertyDevices` will be
//!     called once the bindings are wired.
//!
//! When this is filled in, the call graph should be:
//!
//!   start()
//!     ↳ spawn listener thread
//!         ↳ AudioObjectAddPropertyListener(devices)
//!         ↳ AudioObjectAddPropertyListener(default output)
//!         ↳ CFRunLoopRun()
//!     ↳ initial enumerate via hal_devices()
//!
//!   set_default_output(node_id)
//!     ↳ AudioObjectSetPropertyData(
//!           kAudioObjectSystemObject,
//!           kAudioHardwarePropertyDefaultOutputDevice,
//!           &device_id)
//!
//!   set_volume(node_id, v)
//!     ↳ AudioObjectSetPropertyData(
//!           device_id,
//!           kAudioDevicePropertyVolumeScalar)

use soundworm_core::{
    error::{Result, SoundwormError},
    event::BackendEvent,
    node::{Node, NodeId, NodeKind},
};
use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

/// `AudioDeviceID` is a `u32` in CoreAudio. We widen to `u64` for
/// `NodeId` to match the cross-platform graph schema; the inverse
/// conversion truncates with a safety check.
pub(crate) fn device_id_to_node_id(dev: u32) -> NodeId {
    NodeId(u64::from(dev))
}

pub(crate) fn node_id_to_device_id(id: u64) -> std::result::Result<u32, SoundwormError> {
    u32::try_from(id).map_err(|_| {
        SoundwormError::Backend(format!("node id {id} out of range for AudioDeviceID"))
    })
}

pub(crate) struct Inner {
    nodes: Arc<Mutex<HashMap<u32, Node>>>,
    event_sinks: Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>>,
}

impl Inner {
    pub fn start() -> Result<Self> {
        let nodes = Arc::new(Mutex::new(HashMap::new()));
        let event_sinks: Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>> =
            Arc::new(Mutex::new(Vec::new()));

        // TODO(v0.5-mac): spawn the HAL listener thread that:
        //   1. Calls hal_devices() for the initial snapshot
        //   2. Inserts each Node into `nodes` and broadcasts NodeAppeared
        //   3. Registers an AudioObjectPropertyListener for
        //      kAudioHardwarePropertyDevices and re-syncs on change
        //   4. Runs CFRunLoopRun() to keep the listener alive
        //
        // Mirror the pipewire-backend layout: dedicated non-tokio
        // thread, std::sync::mpsc for events out, no shared mutable
        // state besides `nodes` + `event_sinks` (already Arc<Mutex<…>>).
        tracing::warn!(
            "coreaudio backend: HAL hookup is stubbed — \
             enumerate_nodes will return empty until v0.5 is finished"
        );

        Ok(Self { nodes, event_sinks })
    }

    pub fn subscribe(&self) -> mpsc::Receiver<BackendEvent> {
        let (tx, rx) = mpsc::sync_channel(256);
        self.event_sinks.lock().unwrap().push(tx);
        rx
    }

    pub fn enumerate_nodes(&self) -> Result<Vec<Node>> {
        Ok(self.nodes.lock().unwrap().values().cloned().collect())
    }

    pub fn set_default_output(&self, node_id: u64) -> Result<()> {
        let dev = node_id_to_device_id(node_id)?;
        // TODO(v0.5-mac): AudioObjectSetPropertyData(
        //     kAudioObjectSystemObject,
        //     kAudioHardwarePropertyDefaultOutputDevice,
        //     &dev as *const _ as *const _,
        //     size_of::<AudioDeviceID>())
        tracing::info!("coreaudio set_default_output device={dev} (stub)");
        Ok(())
    }

    pub fn set_volume(&self, node_id: u64, volume: f32) -> Result<()> {
        let dev = node_id_to_device_id(node_id)?;
        let v = volume.clamp(0.0, 1.0);
        // TODO(v0.5-mac): walk the output streams of `dev` and
        // AudioObjectSetPropertyData(kAudioDevicePropertyVolumeScalar)
        // on each. Some devices only expose master volume.
        tracing::info!("coreaudio set_volume device={dev} volume={v} (stub)");
        Ok(())
    }
}

/// Build a [`Node`] from a CoreAudio `AudioDeviceID`. Filled in once
/// the HAL bindings are wired up; for now returns a minimal Node so
/// the type plumbing compiles.
#[allow(dead_code)]
pub(crate) fn node_from_device(dev: u32, name: &str, kind: NodeKind) -> Node {
    Node {
        id: device_id_to_node_id(dev),
        name: name.into(),
        kind,
        app_name: None,
        media_class: String::new(),
        // TODO(v0.5-mac): query kAudioDevicePropertyNominalSampleRate,
        // kAudioDevicePropertyStreams[Input/Output] → channel count,
        // kAudioDevicePropertyLatency → latency_ms.
        sample_rate: 48000,
        channels: 2,
        latency_ms: 0.0,
        properties: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_roundtrips() {
        let id = device_id_to_node_id(42);
        assert_eq!(node_id_to_device_id(id.0).unwrap(), 42);
    }

    #[test]
    fn device_id_overflow_is_caught() {
        let too_big = u64::from(u32::MAX) + 1;
        assert!(node_id_to_device_id(too_big).is_err());
    }
}
