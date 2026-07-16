//! macOS HAL bindings via `coreaudio-sys`. Only compiled on macOS.
//!
//! Working: `enumerate_nodes` (device list → Sink/Source Nodes with name
//! + sample rate) and `set_default_output` (the "link" semantic, since
//! the HAL has no port-to-port linking). Compile-verified on macOS CI;
//! on-hardware runtime still needs a real Mac.
//!
//! Live NodeAppeared/NodeRemoved come from a HAL property listener on
//! `kAudioHardwarePropertyDevices` (see [`Inner::start`]); the HAL is asked
//! to deliver on its own thread (run-loop set to NULL) so no CFRunLoop is
//! needed.

use coreaudio_sys::{
    kAudioDevicePropertyDeviceName, kAudioDevicePropertyMute,
    kAudioDevicePropertyNominalSampleRate, kAudioDevicePropertyStreams,
    kAudioDevicePropertyVolumeScalar, kAudioHardwarePropertyDefaultOutputDevice,
    kAudioHardwarePropertyDevices, kAudioHardwarePropertyRunLoop,
    kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyScopeInput,
    kAudioObjectPropertyScopeOutput, kAudioObjectSystemObject, AudioDeviceID,
    AudioObjectAddPropertyListener, AudioObjectGetPropertyData,
    AudioObjectGetPropertyDataSize, AudioObjectID, AudioObjectPropertyAddress,
    AudioObjectRemovePropertyListener, AudioObjectSetPropertyData, AudioStreamID,
    CFRunLoopRef, OSStatus,
};
use soundworm_core::{
    error::{Result, SoundwormError},
    event::BackendEvent,
    node::{Node, NodeId, NodeKind},
};
use std::collections::{HashMap, HashSet};
use std::os::raw::c_void;
use std::sync::{mpsc, Arc, Mutex};
use std::{mem, ptr};

// Master/Main both equal 0; hardcoding sidesteps the constant rename
// across macOS SDK versions that coreaudio-sys bindgen tracks.
const ELEMENT_MAIN: u32 = 0;

fn address(selector: u32, scope: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress { mSelector: selector, mScope: scope, mElement: ELEMENT_MAIN }
}

/// Byte size the HAL reports for a property, or None on error.
unsafe fn property_size(obj: AudioObjectID, addr: &AudioObjectPropertyAddress) -> Option<usize> {
    let mut size: u32 = 0;
    let st = AudioObjectGetPropertyDataSize(obj, addr, 0, ptr::null(), &mut size);
    (st == 0).then_some(size as usize)
}

/// Read a property whose payload is a packed array of `u32`-sized ids
/// (device ids, stream ids). Returns empty on any HAL error.
unsafe fn read_u32_array(obj: AudioObjectID, addr: &AudioObjectPropertyAddress) -> Vec<u32> {
    let Some(size) = property_size(obj, addr) else { return Vec::new() };
    let count = size / mem::size_of::<u32>();
    if count == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u32; count];
    let mut io = size as u32;
    let st = AudioObjectGetPropertyData(
        obj, addr, 0, ptr::null(), &mut io, buf.as_mut_ptr() as *mut c_void,
    );
    if st != 0 {
        return Vec::new();
    }
    buf.truncate(io as usize / mem::size_of::<u32>());
    buf
}

unsafe fn stream_count(dev: AudioDeviceID, scope: u32) -> usize {
    let addr = address(kAudioDevicePropertyStreams, scope);
    property_size(dev, &addr)
        .map(|s| s / mem::size_of::<AudioStreamID>())
        .unwrap_or(0)
}

unsafe fn device_name(dev: AudioDeviceID) -> String {
    let addr = address(kAudioDevicePropertyDeviceName, kAudioObjectPropertyScopeGlobal);
    let Some(size) = property_size(dev, &addr) else { return String::new() };
    let mut buf = vec![0u8; size];
    let mut io = size as u32;
    let st = AudioObjectGetPropertyData(
        dev, &addr, 0, ptr::null(), &mut io, buf.as_mut_ptr() as *mut c_void,
    );
    if st != 0 {
        return String::new();
    }
    match std::ffi::CStr::from_bytes_until_nul(&buf) {
        Ok(c) => c.to_string_lossy().into_owned(),
        Err(_) => String::new(),
    }
}

unsafe fn nominal_sample_rate(dev: AudioDeviceID) -> u32 {
    let addr = address(kAudioDevicePropertyNominalSampleRate, kAudioObjectPropertyScopeGlobal);
    let mut sr: f64 = 0.0;
    let mut io = mem::size_of::<f64>() as u32;
    let st = AudioObjectGetPropertyData(
        dev, &addr, 0, ptr::null(), &mut io, &mut sr as *mut f64 as *mut c_void,
    );
    if st == 0 && sr > 0.0 {
        sr as u32
    } else {
        48000
    }
}

/// Enumerate the HAL device list into cross-platform `Node`s. A device
/// with output streams is a Sink, with input streams a Source; devices
/// with neither (aggregate control-only) are skipped.
fn hal_devices() -> Vec<Node> {
    unsafe {
        let sys = kAudioObjectSystemObject as AudioObjectID;
        let addr = address(kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal);
        let devices = read_u32_array(sys, &addr);
        devices
            .into_iter()
            .filter_map(|dev| {
                let outputs = stream_count(dev, kAudioObjectPropertyScopeOutput);
                let inputs = stream_count(dev, kAudioObjectPropertyScopeInput);
                let (kind, media_class) = if outputs > 0 {
                    (NodeKind::Sink, "Audio/Sink")
                } else if inputs > 0 {
                    (NodeKind::Source, "Audio/Source")
                } else {
                    return None;
                };
                Some(Node {
                    id: device_id_to_node_id(dev),
                    name: device_name(dev),
                    kind,
                    app_name: None,
                    media_class: media_class.into(),
                    sample_rate: nominal_sample_rate(dev),
                    channels: 2,
                    latency_ms: 0.0,
                    properties: HashMap::new(),
                })
            })
            .collect()
    }
}

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

// Fan an event out to every live subscriber, pruning dead ones.
fn broadcast(sinks: &Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>>, event: BackendEvent) {
    if let Ok(mut g) = sinks.lock() {
        g.retain(|tx| tx.try_send(event.clone()).is_ok());
    }
}

// Context the HAL callback dereferences. Boxed and pinned behind a raw
// pointer for the listener's lifetime (freed in `Inner::drop`).
struct ListenerCtx {
    sinks: Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>>,
    prev: Mutex<HashSet<NodeId>>,
}

// HAL property listener for kAudioHardwarePropertyDevices: re-enumerate,
// diff against the previously-seen id set, and emit NodeAppeared /
// NodeRemoved for the delta.
unsafe extern "C" fn devices_changed_proc(
    _in_object: AudioObjectID,
    _n_addresses: u32,
    _addresses: *const AudioObjectPropertyAddress,
    client: *mut c_void,
) -> OSStatus {
    if client.is_null() {
        return 0;
    }
    // SAFETY: `client` is the ListenerCtx pointer registered in
    // Inner::start; it outlives the listener (freed only after the
    // listener is removed in Inner::drop).
    let ctx = unsafe { &*(client as *const ListenerCtx) };
    let current = hal_devices();
    let cur_ids: HashSet<NodeId> = current.iter().map(|n| n.id.clone()).collect();
    let Ok(mut prev) = ctx.prev.lock() else { return 0 };
    for node in &current {
        if !prev.contains(&node.id) {
            broadcast(&ctx.sinks, BackendEvent::NodeAppeared(node.clone()));
        }
    }
    for id in prev.iter() {
        if !cur_ids.contains(id) {
            broadcast(&ctx.sinks, BackendEvent::NodeRemoved(id.clone()));
        }
    }
    *prev = cur_ids;
    0
}

pub(crate) struct Inner {
    event_sinks: Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>>,
    // The HAL holds a raw pointer to this context, so it must outlive the
    // registration; freed in Drop after the listener is removed.
    ctx: *mut ListenerCtx,
}

// SAFETY: `ctx` is only dereferenced by the HAL callback and freed in Drop
// after the listener is removed; the data behind it is Send + Sync
// (Arc<Mutex<..>> + Mutex<..>).
unsafe impl Send for Inner {}
// SAFETY: see the Send impl above.
unsafe impl Sync for Inner {}

impl Inner {
    pub fn start() -> Result<Self> {
        let event_sinks: Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>> =
            Arc::new(Mutex::new(Vec::new()));
        let initial: HashSet<NodeId> = hal_devices().into_iter().map(|n| n.id).collect();
        let ctx = Box::into_raw(Box::new(ListenerCtx {
            sinks: event_sinks.clone(),
            prev: Mutex::new(initial),
        }));

        let sys = kAudioObjectSystemObject as AudioObjectID;
        let devices_addr = address(kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal);
        // SAFETY: FFI into the HAL with valid addresses and our owned ctx
        // pointer; on failure we reclaim the Box before returning.
        let st = unsafe {
            // Deliver notifications on the HAL's own thread (run loop NULL)
            // instead of requiring a CFRunLoop on this thread.
            let rl_addr = address(kAudioHardwarePropertyRunLoop, kAudioObjectPropertyScopeGlobal);
            let null_rl: CFRunLoopRef = ptr::null_mut();
            AudioObjectSetPropertyData(
                sys,
                &rl_addr,
                0,
                ptr::null(),
                mem::size_of::<CFRunLoopRef>() as u32,
                &null_rl as *const CFRunLoopRef as *const c_void,
            );
            AudioObjectAddPropertyListener(sys, &devices_addr, Some(devices_changed_proc), ctx as *mut c_void)
        };
        if st != 0 {
            // SAFETY: ctx came from Box::into_raw above and was never
            // registered, so reclaiming it here is sound.
            unsafe { drop(Box::from_raw(ctx)) };
            return Err(SoundwormError::Backend(format!(
                "coreaudio: AudioObjectAddPropertyListener failed: OSStatus {st}"
            )));
        }
        Ok(Self { event_sinks, ctx })
    }

    pub fn subscribe(&self) -> mpsc::Receiver<BackendEvent> {
        let (tx, rx) = mpsc::sync_channel(256);
        self.event_sinks.lock().unwrap().push(tx);
        rx
    }

    pub fn enumerate_nodes(&self) -> Result<Vec<Node>> {
        Ok(hal_devices())
    }

    /// CoreAudio has no port-to-port linking, so "route to this sink"
    /// means make it the system default output device.
    pub fn set_default_output(&self, node_id: u64) -> Result<()> {
        let mut dev = node_id_to_device_id(node_id)?;
        let addr = address(
            kAudioHardwarePropertyDefaultOutputDevice,
            kAudioObjectPropertyScopeGlobal,
        );
        let st = unsafe {
            AudioObjectSetPropertyData(
                kAudioObjectSystemObject as AudioObjectID,
                &addr,
                0,
                ptr::null(),
                mem::size_of::<AudioDeviceID>() as u32,
                &mut dev as *mut AudioDeviceID as *const c_void,
            )
        };
        if st != 0 {
            return Err(SoundwormError::Backend(format!(
                "set default output device {dev} failed: OSStatus {st}"
            )));
        }
        Ok(())
    }

    /// Set the device master output volume (0..1). Uses the main element
    /// on the output scope; devices that expose only per-stream volume
    /// return an error, which the caller surfaces.
    pub fn set_volume(&self, node_id: u64, volume: f32) -> Result<()> {
        let dev = node_id_to_device_id(node_id)?;
        let mut v = volume.clamp(0.0, 1.0);
        let addr = address(kAudioDevicePropertyVolumeScalar, kAudioObjectPropertyScopeOutput);
        let st = unsafe {
            AudioObjectSetPropertyData(
                dev,
                &addr,
                0,
                ptr::null(),
                mem::size_of::<f32>() as u32,
                &mut v as *mut f32 as *const c_void,
            )
        };
        if st != 0 {
            return Err(SoundwormError::Backend(format!(
                "set volume device {dev} failed: OSStatus {st}"
            )));
        }
        Ok(())
    }

    pub fn set_mute(&self, node_id: u64, mute: bool) -> Result<()> {
        let dev = node_id_to_device_id(node_id)?;
        let mut m: u32 = mute.into();
        let addr = address(kAudioDevicePropertyMute, kAudioObjectPropertyScopeOutput);
        let st = unsafe {
            AudioObjectSetPropertyData(
                dev,
                &addr,
                0,
                ptr::null(),
                mem::size_of::<u32>() as u32,
                &mut m as *mut u32 as *const c_void,
            )
        };
        if st != 0 {
            return Err(SoundwormError::Backend(format!(
                "set mute device {dev} failed: OSStatus {st}"
            )));
        }
        Ok(())
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let sys = kAudioObjectSystemObject as AudioObjectID;
        let addr = address(kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal);
        // SAFETY: unregister the listener before reclaiming the ctx Box, so
        // the HAL never dereferences a freed pointer.
        unsafe {
            AudioObjectRemovePropertyListener(
                sys,
                &addr,
                Some(devices_changed_proc),
                self.ctx as *mut c_void,
            );
            drop(Box::from_raw(self.ctx));
        }
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
