//! WASAPI endpoint enumeration via the MMDevice API (windows-rs).
//!
//! Compiled only on Windows. Enumerates active render/capture endpoints
//! into cross-platform `Node`s. Endpoint IDs are strings, so `NodeId`
//! (u64) is a stable hash of the endpoint id; the raw id is kept as the
//! node name for now (friendly name via IPropertyStore is a follow-up).

use soundworm_core::node::{Node, NodeId, NodeKind};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use windows::Win32::Media::Audio::{
    eCapture, eRender, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};

fn hash_id(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

pub fn enumerate_endpoints() -> Vec<Node> {
    let mut out = Vec::new();
    unsafe {
        // May already be initialized on this thread; ignore the result.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(_) => return out,
            };

        for (flow, kind, media_class) in [
            (eRender, NodeKind::Sink, "Audio/Sink"),
            (eCapture, NodeKind::Source, "Audio/Source"),
        ] {
            let collection = match enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let count = collection.GetCount().unwrap_or(0);
            for i in 0..count {
                let device = match collection.Item(i) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                let id = match device.GetId() {
                    Ok(pwstr) => {
                        let s = pwstr.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(pwstr.0 as *const _));
                        s
                    }
                    Err(_) => continue,
                };
                if id.is_empty() {
                    continue;
                }
                out.push(Node {
                    id: NodeId(hash_id(&id)),
                    name: id,
                    kind,
                    app_name: None,
                    media_class: media_class.into(),
                    sample_rate: 48000,
                    channels: 2,
                    latency_ms: 0.0,
                    properties: HashMap::new(),
                });
            }
        }
    }
    out
}
