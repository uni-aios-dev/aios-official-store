//! Guest-side helpers for the AIOS WASM IPC ABI.
//!
//! The host serializes `aios_core::ipc_protocol::IpcPacket` with `bincode`
//! version 1, stages it in guest memory and calls `handle_message(ptr, len)`.
//! A block returns a packed `(response_ptr << 32) | response_len`, with zero
//! meaning "no response". This crate mirrors the wire types needed by a block
//! and owns a small bump allocator for the staged request and the response.

use serde::{Deserialize, Serialize};
use std::slice;

/// Command id used for block-to-console messages.
pub const COMMAND_CUSTOM: u16 = 0x00FF;
/// Priority assigned to block responses.
pub const RESPONSE_PRIORITY: u8 = 2;

/// First byte offset handed out by the guest allocator, keeping pointers
/// strictly positive as required by the host ABI.
const ALLOC_START: usize = 1024;
/// Bytes reserved for staged IPC requests and block responses.
const ALLOC_CAPACITY: usize = 1024 * 1024;

static mut IPC_BUFFER: [u8; ALLOC_CAPACITY] = [0; ALLOC_CAPACITY];
static mut IPC_CURSOR: usize = ALLOC_START;

/// Fixed header mirrored from `aios_core::ipc_protocol::Header`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Header {
    /// Packet identifier.
    pub packet_id: u64,
    /// Sending block id.
    pub source_block: u32,
    /// Receiving block id.
    pub target_block: u32,
    /// Command identifier.
    pub command_id: u16,
    /// Scheduling priority.
    pub priority: u8,
    /// Serialized payload length.
    pub payload_len: u32,
    /// Payload checksum; block responses leave it zeroed because the host does
    /// not verify it on the WASM path.
    pub checksum: [u8; 32],
}

/// Wire-compatible subset of the host `Payload` enum.
///
/// Variants `3..=20` exist only to keep `Custom` at host index 21; they are
/// never constructed by a block and are rejected when decoding a request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Payload {
    /// No data.
    Empty,
    /// Opaque bytes.
    Binary(Vec<u8>),
    /// UTF-8 text.
    Text(String),
    /// Registration payload (reserved by the host protocol).
    RegisterBlock,
    /// Unload payload (reserved by the host protocol).
    UnloadBlock,
    /// Topology payload (reserved by the host protocol).
    GetTopology,
    /// Spawn payload (reserved by the host protocol).
    SpawnProcess,
    /// Kill payload (reserved by the host protocol).
    KillProcess,
    /// Priority payload (reserved by the host protocol).
    AdjustPriority,
    /// Health payload (reserved by the host protocol).
    HealthCheck,
    /// State payload (reserved by the host protocol).
    ExtractState,
    /// State payload (reserved by the host protocol).
    RestoreState,
    /// Hot-swap payload (reserved by the host protocol).
    HotSwap,
    /// Rollback payload (reserved by the host protocol).
    Rollback,
    /// Intent payload (reserved by the host protocol).
    IntentCommand,
    /// Cluster payload (reserved by the host protocol).
    ClusterSpawn,
    /// Cluster payload (reserved by the host protocol).
    ClusterKill,
    /// Cluster payload (reserved by the host protocol).
    ClusterStatus,
    /// Cluster payload (reserved by the host protocol).
    ClusterStatusReply,
    /// Fault payload (reserved by the host protocol).
    FaultReport,
    /// Fault payload (reserved by the host protocol).
    RecoveryDecision,
    /// Tagged block message: `(tag, utf8 bytes)`.
    Custom(String, Vec<u8>),
}

/// Wire-compatible packet exchanged with the host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcPacket {
    /// Packet header.
    pub header: Header,
    /// Packet body.
    pub payload: Payload,
}

/// Request decoded from an inbound `IpcPacket`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Tag supplied by the sender, empty for untagged text requests.
    pub tag: String,
    /// Text argument supplied by the console.
    pub text: String,
}

impl Payload {
    /// Serialized payload length used for the response header.
    pub fn encoded_len(&self) -> usize {
        bincode::serialize(self).map_or(0, |bytes| bytes.len())
    }
}

/// Convert a pointer exported by `aios_alloc` into an address usable by this
/// process. On WASM the value is already a linear-memory address; on the
/// native test target it is an offset into `IPC_BUFFER`.
fn imported_ptr(ptr: i32) -> *mut u8 {
    #[cfg(target_arch = "wasm32")]
    {
        ptr as *mut u8
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        unsafe { buffer_base().add(ptr as usize) }
    }
}

/// Convert a buffer offset into the pointer value exported to the host.
fn exported_ptr(offset: usize) -> i32 {
    #[cfg(target_arch = "wasm32")]
    {
        (buffer_base() as usize + offset) as i32
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        offset as i32
    }
}

fn buffer_base() -> *mut u8 {
    std::ptr::addr_of_mut!(IPC_BUFFER) as *mut u8
}

/// Decode the request staged at `ptr` with length `len`.
pub fn decode_request(ptr: i32, len: i32) -> Result<Request, &'static str> {
    if ptr <= 0 || len < 0 {
        return Err("invalid request pointer or length");
    }
    let bytes = unsafe { slice::from_raw_parts(imported_ptr(ptr) as *const u8, len as usize) };
    let packet: IpcPacket = bincode::deserialize(bytes).map_err(|_| "invalid IPC packet")?;
    match packet.payload {
        Payload::Custom(tag, data) => Ok(Request {
            tag,
            text: String::from_utf8_lossy(&data).into_owned(),
        }),
        Payload::Text(text) => Ok(Request {
            tag: String::new(),
            text,
        }),
        Payload::Binary(data) => Ok(Request {
            tag: String::new(),
            text: String::from_utf8_lossy(&data).into_owned(),
        }),
        Payload::Empty => Ok(Request {
            tag: String::new(),
            text: String::new(),
        }),
        _ => Err("unsupported payload variant"),
    }
}

/// Encode `text` as a response packet and return the packed ABI pointer.
pub fn encode_response(text: &str) -> i64 {
    let payload = Payload::Text(text.to_string());
    let payload_len = payload.encoded_len();
    let packet = IpcPacket {
        header: Header {
            packet_id: 0,
            source_block: 0,
            target_block: 0,
            command_id: COMMAND_CUSTOM,
            priority: RESPONSE_PRIORITY,
            payload_len: payload_len as u32,
            checksum: [0; 32],
        },
        payload,
    };
    let bytes = match bincode::serialize(&packet) {
        Ok(bytes) => bytes,
        Err(_) => return 0,
    };
    let offset = unsafe {
        match alloc_offset(bytes.len()) {
            Ok(offset) => offset,
            Err(_) => return 0,
        }
    };
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer_base().add(offset), bytes.len());
    }
    let ptr = exported_ptr(offset);
    (((ptr as u64) << 32) | (bytes.len() as u64)) as i64
}

/// Decode an inbound request, call `handler` and encode its reply.
///
/// `aios_alloc` resets the bump cursor before the host stages each request, so
/// the response is written immediately after the request without overwriting
/// it before decoding has finished.
pub fn process_request<F>(ptr: i32, len: i32, handler: F) -> i64
where
    F: FnOnce(&Request) -> String,
{
    let request = match decode_request(ptr, len) {
        Ok(request) => request,
        Err(error) => return encode_response(&format!("error: {error}")),
    };
    let response = handler(&request);
    encode_response(&response)
}

/// Host-called allocator export. Resets the bump cursor for a fresh request.
///
/// # Safety
///
/// The host writes `len` bytes at the returned offset in this module's linear
/// memory before calling `handle_message`.
#[no_mangle]
pub extern "C" fn aios_alloc(len: i32) -> i32 {
    unsafe {
        IPC_CURSOR = ALLOC_START;
        if len <= 0 {
            return exported_ptr(ALLOC_START);
        }
        match alloc_offset(len as usize) {
            Ok(offset) => exported_ptr(offset),
            Err(_) => 0,
        }
    }
}

/// Optional lifecycle export called once after instantiation.
#[no_mangle]
pub extern "C" fn init() {}

/// Optional lifecycle export called once after `init`.
#[no_mangle]
pub extern "C" fn start() {}

unsafe fn alloc_offset(len: usize) -> Result<usize, &'static str> {
    if len == 0 || len > ALLOC_CAPACITY - ALLOC_START {
        return Err("allocation exceeds block IPC buffer");
    }
    let offset = IPC_CURSOR;
    IPC_CURSOR = offset + len;
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet_with_payload(payload: Payload) -> Vec<u8> {
        bincode::serialize(&IpcPacket {
            header: Header {
                packet_id: 1,
                source_block: 0,
                target_block: 1,
                command_id: COMMAND_CUSTOM,
                priority: RESPONSE_PRIORITY,
                payload_len: payload.encoded_len() as u32,
                checksum: [0; 32],
            },
            payload,
        })
        .unwrap()
    }

    #[test]
    fn custom_variant_index_matches_host() {
        let bytes = bincode::serialize(&Payload::Custom("x".into(), vec![7])).unwrap();
        assert_eq!(&bytes[..4], &21u32.to_le_bytes());
        let text = bincode::serialize(&Payload::Text("x".into())).unwrap();
        assert_eq!(&text[..4], &2u32.to_le_bytes());
    }

    #[test]
    fn decode_custom_request() {
        let bytes = packet_with_payload(Payload::Custom("calculator".into(), b"2+3".to_vec()));
        let ptr = aios_alloc(bytes.len() as i32);
        assert!(ptr > 0);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), imported_ptr(ptr), bytes.len());
        }
        let request = decode_request(ptr, bytes.len() as i32).unwrap();
        assert_eq!(request.tag, "calculator");
        assert_eq!(request.text, "2+3");
    }

    #[test]
    fn response_roundtrips_through_host_wire_format() {
        let packed = encode_response("answer: 5");
        assert_ne!(packed, 0);
        let ptr = (packed >> 32) as i32;
        let len = (packed & 0xFFFF_FFFF) as usize;
        let bytes = unsafe { slice::from_raw_parts(imported_ptr(ptr) as *const u8, len) };
        let packet: IpcPacket = bincode::deserialize(bytes).unwrap();
        assert_eq!(packet.payload, Payload::Text("answer: 5".into()));
        assert_eq!(packet.header.command_id, COMMAND_CUSTOM);
    }
}
