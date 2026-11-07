//! CONTRACT-to-CALLGRAPH synchronous UDP bridge.
//! Wire stack: CALLGRAPH frame inside OCTET frame inside DATAGRAM envelope.
//! Correlation identifiers are unique within this process; they are not credentials.
use std::{
    io,
    net::{SocketAddr, UdpSocket},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
const DGM_HEADER: usize = 24;
const OCT_HEADER: usize = 10;
const RPC_HEADER: usize = 38;
const KIND_REQUEST: u8 = 1;
const KIND_RESPONSE: u8 = 2;
const KIND_ERROR: u8 = 3;

#[derive(Clone, Copy, Debug)]
pub struct RpcConfig {
    pub max_payload_bytes: usize,
    pub max_datagram_bytes: usize,
    pub timeout: Duration,
    pub max_retries: u32,
    pub retry_interval: Duration,
}
impl Default for RpcConfig {
    fn default() -> Self {
        Self { max_payload_bytes: 60 * 1024, max_datagram_bytes: 65_507, timeout: Duration::from_secs(5), max_retries: 2, retry_interval: Duration::from_millis(250) }
    }
}
#[derive(Debug)]
pub enum RpcError {
    Io(io::Error),
    InvalidConfig,
    PayloadTooLarge,
    MalformedDatagram,
    MalformedOctet,
    MalformedRpcFrame,
    RequestMismatch,
    UnexpectedMessage,
    Remote(String),
}
impl From<io::Error> for RpcError { fn from(value: io::Error) -> Self { Self::Io(value) } }
impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "RPC I/O error: {e}"),
            Self::InvalidConfig => write!(f, "invalid RPC client configuration"),
            Self::PayloadTooLarge => write!(f, "RPC payload exceeds configured limit"),
            Self::MalformedDatagram => write!(f, "malformed DATAGRAM envelope"),
            Self::MalformedOctet => write!(f, "malformed OCTET frame"),
            Self::MalformedRpcFrame => write!(f, "malformed CALLGRAPH frame"),
            Self::RequestMismatch => write!(f, "RPC response request ID mismatch"),
            Self::UnexpectedMessage => write!(f, "unexpected RPC message kind"),
            Self::Remote(message) => write!(f, "remote operation failed: {message}"),
        }
    }
}
impl std::error::Error for RpcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self { Self::Io(e) => Some(e), _ => None }
    }
}

pub struct RpcClient { remote: SocketAddr, config: RpcConfig }
impl RpcClient {
    pub fn new(remote: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        if config.max_payload_bytes == 0 || config.timeout.is_zero() || config.retry_interval.is_zero()
            || config.max_datagram_bytes > 65_507
            || config.max_datagram_bytes < DGM_HEADER + OCT_HEADER + RPC_HEADER
            || config.max_payload_bytes > config.max_datagram_bytes - DGM_HEADER - OCT_HEADER - RPC_HEADER {
            return Err(RpcError::InvalidConfig);
        }
        Ok(Self { remote, config })
    }

    pub fn call(&self, operation_id: u32, major: u16, minor: u16, payload: &[u8]) -> Result<Vec<u8>, RpcError> {
        if payload.len() > self.config.max_payload_bytes { return Err(RpcError::PayloadTooLarge); }
        let socket = UdpSocket::bind(if self.remote.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
        socket.connect(self.remote)?;
        socket.set_write_timeout(Some(self.config.timeout))?;

        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
        let mut request_id = [0u8; 16];
        request_id[..8].copy_from_slice(&now.to_be_bytes());
        request_id[8..].copy_from_slice(&id.to_be_bytes());
        let session = now ^ id.rotate_left(17);

        let mut rpc = vec![0u8; RPC_HEADER + payload.len()];
        rpc[..4].copy_from_slice(b"CGR1");
        rpc[4] = 1; rpc[5] = KIND_REQUEST;
        rpc[6..22].copy_from_slice(&request_id);
        rpc[22..26].copy_from_slice(&operation_id.to_be_bytes());
        rpc[26..28].copy_from_slice(&major.to_be_bytes());
        rpc[28..30].copy_from_slice(&minor.to_be_bytes());
        let deadline_ms = self.config.timeout.as_millis().min(u32::MAX as u128) as u32;
        rpc[30..34].copy_from_slice(&deadline_ms.to_be_bytes());
        rpc[34..38].copy_from_slice(&(payload.len() as u32).to_be_bytes());
        rpc[38..].copy_from_slice(payload);

        let mut octet = vec![0u8; OCT_HEADER + rpc.len()];
        octet[..4].copy_from_slice(b"OCT1"); octet[4] = 1;
        octet[6..10].copy_from_slice(&(rpc.len() as u32).to_be_bytes());
        octet[10..].copy_from_slice(&rpc);

        let mut packet = vec![0u8; DGM_HEADER + octet.len()];
        packet[..4].copy_from_slice(b"DGM1"); packet[4] = 1;
        packet[6..14].copy_from_slice(&session.to_be_bytes());
        packet[14..22].copy_from_slice(&1u64.to_be_bytes());
        packet[22..24].copy_from_slice(&(octet.len() as u16).to_be_bytes());
        packet[24..].copy_from_slice(&octet);
        if packet.len() > self.config.max_datagram_bytes { return Err(RpcError::PayloadTooLarge); }
        let total_deadline = std::time::Instant::now() + self.config.timeout;
        let mut response = vec![0u8; self.config.max_datagram_bytes + 1];
        let mut attempts = 0u32;
        let n = loop {
            if attempts > self.config.max_retries {
                return Err(RpcError::Io(io::Error::new(io::ErrorKind::TimedOut, "RPC retry budget exhausted")));
            }
            attempts += 1;
            let now = std::time::Instant::now();
            if now >= total_deadline { return Err(RpcError::Io(io::Error::new(io::ErrorKind::TimedOut, "RPC deadline elapsed"))); }
            let wait = self.config.retry_interval.min(total_deadline.saturating_duration_since(now));
            socket.set_read_timeout(Some(wait))?;
            socket.send(&packet)?;
            match socket.recv(&mut response) {
                Ok(n) => break n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
                    if std::time::Instant::now() >= total_deadline { return Err(RpcError::Io(e)); }
                    // The same request ID/session/sequence is retransmitted.
                    // CALLGRAPH deduplicates it and replays a cached response.
                }
                Err(e) => return Err(RpcError::Io(e)),
            }
        };
        if n > self.config.max_datagram_bytes { return Err(RpcError::MalformedDatagram); }
        let response = &response[..n];
        if response.len() < DGM_HEADER || &response[..4] != b"DGM1" || response[4] != 1
            || u64::from_be_bytes(response[6..14].try_into().unwrap()) != session
            || u64::from_be_bytes(response[14..22].try_into().unwrap()) != 1
            || response.len() != DGM_HEADER + u16::from_be_bytes(response[22..24].try_into().unwrap()) as usize {
            return Err(RpcError::MalformedDatagram);
        }
        let octet = &response[DGM_HEADER..];
        if octet.len() < OCT_HEADER || &octet[..4] != b"OCT1" || octet[4] != 1
            || octet.len() != OCT_HEADER + u32::from_be_bytes(octet[6..10].try_into().unwrap()) as usize {
            return Err(RpcError::MalformedOctet);
        }
        let frame = &octet[OCT_HEADER..];
        if frame.len() < RPC_HEADER || &frame[..4] != b"CGR1" || frame[4] != 1
            || frame.len() != RPC_HEADER + u32::from_be_bytes(frame[34..38].try_into().unwrap()) as usize {
            return Err(RpcError::MalformedRpcFrame);
        }
        if frame[6..22] != request_id { return Err(RpcError::RequestMismatch); }
        let body = frame[RPC_HEADER..].to_vec();
        match frame[5] {
            KIND_RESPONSE => Ok(body),
            KIND_ERROR => Err(RpcError::Remote(String::from_utf8_lossy(&body).into_owned())),
            _ => Err(RpcError::UnexpectedMessage),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::UdpSocket, thread};

    #[test]
    fn talks_to_callgraph_wire_server() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let mut buf = [0u8; 2048];
            let (n, peer) = server.recv_from(&mut buf).unwrap();
            let request = &buf[..n];
            assert_eq!(&request[..4], b"DGM1");
            let octet = &request[DGM_HEADER..];
            assert_eq!(&octet[..4], b"OCT1");
            let frame = &octet[OCT_HEADER..];
            assert_eq!(&frame[..4], b"CGR1");
            assert_eq!(u32::from_be_bytes(frame[22..26].try_into().unwrap()), 42);
            let payload = &frame[RPC_HEADER..];
            let response_body = [payload, b" received"].concat();
            let mut response_frame = vec![0u8; RPC_HEADER + response_body.len()];
            response_frame[..4].copy_from_slice(b"CGR1"); response_frame[4] = 1; response_frame[5] = KIND_RESPONSE;
            response_frame[6..22].copy_from_slice(&frame[6..22]);
            response_frame[22..26].copy_from_slice(&42u32.to_be_bytes());
            response_frame[26..28].copy_from_slice(&1u16.to_be_bytes());
            response_frame[34..38].copy_from_slice(&(response_body.len() as u32).to_be_bytes());
            response_frame[38..].copy_from_slice(&response_body);
            let mut response_octet = vec![0u8; OCT_HEADER + response_frame.len()];
            response_octet[..4].copy_from_slice(b"OCT1"); response_octet[4] = 1;
            response_octet[6..10].copy_from_slice(&(response_frame.len() as u32).to_be_bytes());
            response_octet[10..].copy_from_slice(&response_frame);
            let mut response = vec![0u8; DGM_HEADER + response_octet.len()];
            response[..4].copy_from_slice(b"DGM1"); response[4] = 1;
            response[6..14].copy_from_slice(&request[6..14]);
            response[14..22].copy_from_slice(&request[14..22]);
            response[22..24].copy_from_slice(&(response_octet.len() as u16).to_be_bytes());
            response[24..].copy_from_slice(&response_octet);
            server.send_to(&response, peer).unwrap();
        });
        let client = RpcClient::new(addr, RpcConfig::default()).unwrap();
        assert_eq!(client.call(42, 1, 0, b"ping").unwrap(), b"ping received");
        worker.join().unwrap();
    }

    #[test]
    fn rejects_inconsistent_wire_limits() {
        let config = RpcConfig { max_payload_bytes: 100_000, ..RpcConfig::default() };
        assert!(matches!(RpcClient::new("127.0.0.1:9".parse().unwrap(), config), Err(RpcError::InvalidConfig)));
    }
}
