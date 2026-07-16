//! WASAPI backend internals via the MMDevice API (windows-rs).
//!
//! Compiled only on Windows. Covers enumeration (active render/capture
//! endpoints with friendly names), routing (set the default endpoint),
//! per-endpoint volume, and live device notifications.
//!
//! Endpoint IDs are strings, so `NodeId` (u64) is a stable hash of the
//! endpoint id. Routing therefore re-enumerates to map a hash back to
//! its endpoint id before acting.
//!
//! # Routing needs an undocumented interface
//!
//! Windows has no public API to set the default endpoint. The system
//! uses `IPolicyConfig` (CLSID `PolicyConfigClient`), an undocumented
//! COM interface that the Control Panel sound applet drives. We declare
//! its vtable by hand up to `SetDefaultEndpoint`. This is the same
//! mechanism third-party routers use; it is stable across Win7..Win11
//! but not contractually guaranteed by Microsoft.
//!
//! Compile-verified against the `x86_64-pc-windows-msvc` target. On-
//! hardware runtime behaviour still needs a real Windows box.

// The hand-declared IPolicyConfig COM methods keep their PascalCase
// vtable names, as windows-rs's own generated bindings do.
#![allow(non_snake_case)]

use core::ffi::c_void;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ptr;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

use soundworm_core::{
    error::{Result, SoundwormError},
    event::BackendEvent,
    node::{Node, NodeId, NodeKind},
};

use windows::core::{implement, interface, IUnknown, IUnknown_Vtbl, Interface, GUID, HRESULT, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eAll, eCapture, eCommunications, eConsole, eMultimedia, eRender, EDataFlow, ERole, IMMDevice,
    IMMDeviceEnumerator, IMMEndpoint, IMMNotificationClient, IMMNotificationClient_Impl,
    MMDeviceEnumerator, DEVICE_STATE, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};

// PolicyConfigClient CLSID {870af99c-171d-4f9e-af0d-e63df40c2bc9}.
const CLSID_POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

// IPolicyConfig {f8679f50-850a-41cf-9c72-430f290290c8}. Only
// SetDefaultEndpoint is called; the earlier slots are declared with
// raw pointer args purely to place SetDefaultEndpoint at the right
// vtable offset. Every leading param is pointer-sized, so the ABI lines
// up without needing the real WAVEFORMATEX / PROPVARIANT shapes.
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    unsafe fn GetMixFormat(&self, device: PCWSTR, format: *mut *mut c_void) -> HRESULT;
    unsafe fn GetDeviceFormat(
        &self,
        device: PCWSTR,
        default: i32,
        format: *mut *mut c_void,
    ) -> HRESULT;
    unsafe fn ResetDeviceFormat(&self, device: PCWSTR) -> HRESULT;
    unsafe fn SetDeviceFormat(
        &self,
        device: PCWSTR,
        endpoint_format: *mut c_void,
        mix_format: *mut c_void,
    ) -> HRESULT;
    unsafe fn GetProcessingPeriod(
        &self,
        device: PCWSTR,
        default: i32,
        default_period: *mut i64,
        min_period: *mut i64,
    ) -> HRESULT;
    unsafe fn SetProcessingPeriod(&self, device: PCWSTR, period: *mut i64) -> HRESULT;
    unsafe fn GetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
    unsafe fn SetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
    unsafe fn GetPropertyValue(
        &self,
        device: PCWSTR,
        store: i32,
        key: *const c_void,
        value: *mut c_void,
    ) -> HRESULT;
    unsafe fn SetPropertyValue(
        &self,
        device: PCWSTR,
        store: i32,
        key: *const c_void,
        value: *mut c_void,
    ) -> HRESULT;
    unsafe fn SetDefaultEndpoint(&self, device: PCWSTR, role: ERole) -> HRESULT;
    unsafe fn SetEndpointVisibility(&self, device: PCWSTR, visible: i32) -> HRESULT;
}

pub(crate) fn hash_id(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn com_err(e: windows::core::Error) -> SoundwormError {
    SoundwormError::Backend(format!("wasapi: {e}"))
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// COM may already be initialized on this thread; the redundant call is
// harmless (returns S_FALSE), so the result is ignored.
unsafe fn init_com() {
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
}

unsafe fn create_enumerator() -> Result<IMMDeviceEnumerator> {
    init_com();
    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(com_err)
}

unsafe fn pcwstr_to_string(p: PCWSTR) -> String {
    if p.is_null() {
        String::new()
    } else {
        p.to_string().unwrap_or_default()
    }
}

unsafe fn device_id(device: &IMMDevice) -> Option<String> {
    let pwstr = device.GetId().ok()?;
    let s = pcwstr_to_string(PCWSTR(pwstr.0));
    CoTaskMemFree(Some(pwstr.0 as *const c_void));
    (!s.is_empty()).then_some(s)
}

unsafe fn friendly_name(device: &IMMDevice) -> Option<String> {
    let store = device.OpenPropertyStore(STGM_READ).ok()?;
    let mut prop = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
    let name = PropVariantToStringAlloc(&prop).ok().map(|pwstr| {
        let s = pcwstr_to_string(PCWSTR(pwstr.0));
        CoTaskMemFree(Some(pwstr.0 as *const c_void));
        s
    });
    let _ = PropVariantClear(&mut prop);
    name.filter(|s| !s.is_empty())
}

unsafe fn node_from_device(device: &IMMDevice) -> Option<Node> {
    let flow = device.cast::<IMMEndpoint>().ok()?.GetDataFlow().ok()?;
    let (kind, media_class) = match flow {
        f if f == eRender => (NodeKind::Sink, "Audio/Sink"),
        f if f == eCapture => (NodeKind::Source, "Audio/Source"),
        _ => return None,
    };
    let id = device_id(device)?;
    let name = friendly_name(device).unwrap_or_else(|| id.clone());
    Some(Node {
        id: NodeId(hash_id(&id)),
        name,
        kind,
        app_name: None,
        media_class: media_class.into(),
        sample_rate: 48000,
        channels: 2,
        latency_ms: 0.0,
        properties: HashMap::new(),
    })
}

pub(crate) fn enumerate_endpoints() -> Vec<Node> {
    let mut out = Vec::new();
    unsafe {
        let enumerator = match create_enumerator() {
            Ok(e) => e,
            Err(_) => return out,
        };
        let collection = match enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE) {
            Ok(c) => c,
            Err(_) => return out,
        };
        let count = collection.GetCount().unwrap_or(0);
        for i in 0..count {
            if let Ok(device) = collection.Item(i) {
                if let Some(node) = node_from_device(&device) {
                    out.push(node);
                }
            }
        }
    }
    out
}

// Re-enumerate to recover the endpoint id string whose hash matches the
// node id carried in a `Link` / `set_volume` call.
unsafe fn endpoint_id_for_hash(enumerator: &IMMDeviceEnumerator, target: u64) -> Option<String> {
    let collection = enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE).ok()?;
    let count = collection.GetCount().unwrap_or(0);
    for i in 0..count {
        if let Ok(device) = collection.Item(i) {
            if let Some(id) = device_id(&device) {
                if hash_id(&id) == target {
                    return Some(id);
                }
            }
        }
    }
    None
}

// Routing: set the sink endpoint as the default for every role. Windows
// has no port-to-port linking, so a link means "make this the default
// output/input" (the coreaudio backend uses the same model).
pub(crate) fn set_default_endpoint(target: u64) -> Result<()> {
    unsafe {
        let enumerator = create_enumerator()?;
        let id = endpoint_id_for_hash(&enumerator, target).ok_or_else(|| {
            SoundwormError::Backend(format!("wasapi: no endpoint for node {target}"))
        })?;
        let policy: IPolicyConfig =
            CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL).map_err(com_err)?;
        let wide = to_wide(&id);
        for role in [eConsole, eMultimedia, eCommunications] {
            policy
                .SetDefaultEndpoint(PCWSTR(wide.as_ptr()), role)
                .ok()
                .map_err(com_err)?;
        }
    }
    Ok(())
}

pub(crate) fn set_endpoint_volume(target: u64, volume: f32) -> Result<()> {
    unsafe {
        let enumerator = create_enumerator()?;
        let id = endpoint_id_for_hash(&enumerator, target).ok_or_else(|| {
            SoundwormError::Backend(format!("wasapi: no endpoint for node {target}"))
        })?;
        let wide = to_wide(&id);
        let device = enumerator.GetDevice(PCWSTR(wide.as_ptr())).map_err(com_err)?;
        let endpoint_volume: IAudioEndpointVolume =
            device.Activate(CLSCTX_ALL, None).map_err(com_err)?;
        endpoint_volume
            .SetMasterVolumeLevelScalar(volume.clamp(0.0, 1.0), ptr::null())
            .map_err(com_err)?;
    }
    Ok(())
}

// Live device notifications. The MMDevice API delivers these on a system
// MTA thread; the callback builds a `Node` (via the enumerator it holds)
// and fans it out to every subscriber channel. This mirrors the official
// "device events" sample, which also calls GetDevice / OpenPropertyStore
// from inside the callback.
#[implement(IMMNotificationClient)]
struct NotifyClient {
    sinks: Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>>,
    enumerator: IMMDeviceEnumerator,
}

impl NotifyClient {
    fn broadcast(&self, event: BackendEvent) {
        self.sinks
            .lock()
            .unwrap()
            .retain(|tx| tx.try_send(event.clone()).is_ok());
    }

    fn node_for(&self, id: &str) -> Option<Node> {
        unsafe {
            let wide = to_wide(id);
            let device = self.enumerator.GetDevice(PCWSTR(wide.as_ptr())).ok()?;
            node_from_device(&device)
        }
    }
}

impl IMMNotificationClient_Impl for NotifyClient_Impl {
    fn OnDeviceStateChanged(
        &self,
        id: &PCWSTR,
        new_state: DEVICE_STATE,
    ) -> windows::core::Result<()> {
        let id = unsafe { pcwstr_to_string(*id) };
        if new_state == DEVICE_STATE_ACTIVE {
            if let Some(node) = self.node_for(&id) {
                self.broadcast(BackendEvent::NodeAppeared(node));
            }
        } else {
            self.broadcast(BackendEvent::NodeRemoved(NodeId(hash_id(&id))));
        }
        Ok(())
    }

    fn OnDeviceAdded(&self, id: &PCWSTR) -> windows::core::Result<()> {
        let id = unsafe { pcwstr_to_string(*id) };
        if let Some(node) = self.node_for(&id) {
            self.broadcast(BackendEvent::NodeAppeared(node));
        }
        Ok(())
    }

    fn OnDeviceRemoved(&self, id: &PCWSTR) -> windows::core::Result<()> {
        let id = unsafe { pcwstr_to_string(*id) };
        self.broadcast(BackendEvent::NodeRemoved(NodeId(hash_id(&id))));
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        _flow: EDataFlow,
        _role: ERole,
        _id: &PCWSTR,
    ) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnPropertyValueChanged(
        &self,
        _id: &PCWSTR,
        _key: &PROPERTYKEY,
    ) -> windows::core::Result<()> {
        Ok(())
    }
}

// COM interface pointers are not Send, so the enumerator + notification
// client live on a dedicated thread that owns them and keeps the callback
// registered until shutdown. `Inner` itself holds only Send types, so the
// backend stays `Send + Sync`. Notifications are delivered by the MMDevice
// runtime on its own MTA threads straight into `sinks`; this thread just
// keeps the objects alive and parks until dropped.
pub(crate) struct Inner {
    sinks: Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>>,
    stop: Option<mpsc::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl Inner {
    pub fn start() -> Result<Self> {
        let sinks: Arc<Mutex<Vec<mpsc::SyncSender<BackendEvent>>>> =
            Arc::new(Mutex::new(Vec::new()));
        let sinks_worker = sinks.clone();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let worker = thread::spawn(move || unsafe {
            let enumerator = match create_enumerator() {
                Ok(e) => e,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let client: IMMNotificationClient = NotifyClient {
                sinks: sinks_worker,
                enumerator: enumerator.clone(),
            }
            .into();
            if let Err(e) = enumerator.RegisterEndpointNotificationCallback(&client) {
                let _ = ready_tx.send(Err(com_err(e)));
                return;
            }
            let _ = ready_tx.send(Ok(()));
            // Park until Inner is dropped, then unregister and exit.
            let _ = stop_rx.recv();
            let _ = enumerator.UnregisterEndpointNotificationCallback(&client);
        });

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self { sinks, stop: Some(stop_tx), worker: Some(worker) }),
            Ok(Err(e)) => {
                let _ = worker.join();
                Err(e)
            }
            Err(_) => Err(SoundwormError::Backend(
                "wasapi: notification thread exited before init".into(),
            )),
        }
    }

    pub fn subscribe(&self) -> mpsc::Receiver<BackendEvent> {
        let (tx, rx) = mpsc::sync_channel(256);
        self.sinks.lock().unwrap().push(tx);
        rx
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
