//! Provider-neutral synchronous HTTP client for External Agent Protocol v1.0.
//!
//! This module is intentionally a transport adapter. It does not add fields
//! to protocol envelopes or implement any authorization/business semantics.
//! The client uses a bounded `std::net::TcpStream` so it can also be used by
//! validation tools without adding an HTTP client dependency.

use pong_core::http_transport::HTTP_PROTOCOL_PATH;
use pong_core::protocol::{ProtocolRequest, ProtocolResponse, EXTERNAL_AGENT_PROTOCOL_VERSION};
use serde_json::Value;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

/// Conservative default for a response received by the validation client.
pub const DEFAULT_REMOTE_HTTP_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
/// Conservative default for a request read from the client CLI.
pub const DEFAULT_REMOTE_HTTP_MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;

/// Errors from the transport client. The display text is deliberately safe to
/// show to an external caller and never contains a credential or raw OS error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteHttpClientError {
    code: &'static str,
    message: &'static str,
    http_status: Option<u16>,
}

impl RemoteHttpClientError {
    const fn new(code: &'static str, message: &'static str) -> Self {
        Self {
            code,
            message,
            http_status: None,
        }
    }

    const fn with_http_status(mut self, http_status: u16) -> Self {
        self.http_status = Some(http_status);
        self
    }

    /// Stable, bounded error code suitable for JSONL tooling.
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// Stable, secret-free error message suitable for JSONL tooling.
    pub const fn message(&self) -> &'static str {
        self.message
    }

    /// The received HTTP status when a response was received before the
    /// client detected an invalid response envelope.
    pub const fn http_status(&self) -> Option<u16> {
        self.http_status
    }
}

impl fmt::Display for RemoteHttpClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RemoteHttpClientError {}

/// Client settings. `endpoint` may be `host:port` or `http://host:port`.
/// HTTPS is intentionally rejected; TLS belongs at an external terminator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteHttpClientConfig {
    pub endpoint: String,
    pub credential: String,
    pub timeout: Duration,
    pub max_response_bytes: usize,
}

/// A parsed response from the HTTP transport. `body` is present only when the
/// body is valid JSON; malformed/non-JSON bodies remain bounded and are not
/// echoed by the CLI. `response` is present only for a valid Protocol v1.0
/// response whose request id matches the request that was sent.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteHttpReply {
    pub http_status: u16,
    pub body: Option<Value>,
    pub response: Option<ProtocolResponse>,
}

/// A small synchronous HTTP/1.1 client used by independent-process tests and
/// the `pong-agent-http-client` validation binary.
#[derive(Clone, Debug)]
pub struct RemoteHttpClient {
    endpoint: ParsedEndpoint,
    credential: String,
    timeout: Duration,
    max_response_bytes: usize,
}

impl RemoteHttpClient {
    pub fn from_config(config: RemoteHttpClientConfig) -> Result<Self, RemoteHttpClientError> {
        if config.credential.trim().is_empty() || config.credential.chars().any(char::is_whitespace)
        {
            return Err(RemoteHttpClientError::new(
                "INVALID_CREDENTIAL",
                "credential is invalid",
            ));
        }
        if config.timeout.is_zero() {
            return Err(RemoteHttpClientError::new(
                "INVALID_TIMEOUT",
                "client timeout is invalid",
            ));
        }
        if config.max_response_bytes == 0 {
            return Err(RemoteHttpClientError::new(
                "INVALID_RESPONSE_LIMIT",
                "response limit is invalid",
            ));
        }
        let endpoint = ParsedEndpoint::parse(&config.endpoint)?;
        Ok(Self {
            endpoint,
            credential: config.credential,
            timeout: config.timeout,
            max_response_bytes: config.max_response_bytes,
        })
    }

    /// Sends a normal Protocol v1.0 request over a fresh TCP connection.
    pub fn request(
        &self,
        request: &ProtocolRequest,
    ) -> Result<RemoteHttpReply, RemoteHttpClientError> {
        let body = serde_json::to_vec(request).map_err(|_| {
            RemoteHttpClientError::new(
                "REQUEST_SERIALIZATION_FAILED",
                "request is not serializable",
            )
        })?;
        let reply = self.send_json_body(&body)?;
        validate_protocol_reply(reply, &request.request_id)
    }

    /// Sends a JSON body without requiring it to be a Protocol v1.0 envelope.
    /// This is useful for malformed-envelope and payload-limit conformance
    /// checks. No protocol response validation is performed.
    pub fn send_json_body(&self, body: &[u8]) -> Result<RemoteHttpReply, RemoteHttpClientError> {
        self.send_request_bytes(&build_json_request(&self.endpoint, &self.credential, body)?)
    }

    /// Sends a complete HTTP request supplied by a validation harness. The
    /// bytes are written unchanged; this is deliberately not a protocol API.
    pub fn send_raw_http(
        &self,
        request_bytes: &[u8],
    ) -> Result<RemoteHttpReply, RemoteHttpClientError> {
        self.send_request_bytes(request_bytes)
    }

    /// Writes a complete HTTP request and closes the stream without reading a
    /// response. This models a client that loses its connection after sending
    /// an operation while leaving operation recovery to the Core protocol.
    pub fn send_without_reading(&self, request_bytes: &[u8]) -> Result<(), RemoteHttpClientError> {
        let mut stream = self.connect()?;
        stream
            .write_all(request_bytes)
            .and_then(|()| stream.flush())
            .map_err(|_| {
                RemoteHttpClientError::new("HTTP_WRITE_FAILED", "HTTP request could not be sent")
            })?;
        stream.shutdown(Shutdown::Write).map_err(|_| {
            RemoteHttpClientError::new("HTTP_WRITE_FAILED", "HTTP request could not be sent")
        })
    }

    /// Sends a JSON body and closes without reading the response.
    pub fn send_json_without_reading(&self, body: &[u8]) -> Result<(), RemoteHttpClientError> {
        let request = build_json_request(&self.endpoint, &self.credential, body)?;
        self.send_without_reading(&request)
    }

    fn connect(&self) -> Result<TcpStream, RemoteHttpClientError> {
        let stream =
            TcpStream::connect_timeout(&self.endpoint.addr, self.timeout).map_err(|_| {
                RemoteHttpClientError::new("HTTP_CONNECT_FAILED", "HTTP endpoint is unavailable")
            })?;
        stream
            .set_read_timeout(Some(self.timeout))
            .and_then(|()| stream.set_write_timeout(Some(self.timeout)))
            .map_err(|_| {
                RemoteHttpClientError::new(
                    "HTTP_TIMEOUT_CONFIG_FAILED",
                    "HTTP timeout could not be configured",
                )
            })?;
        Ok(stream)
    }

    fn send_request_bytes(
        &self,
        request_bytes: &[u8],
    ) -> Result<RemoteHttpReply, RemoteHttpClientError> {
        let mut stream = self.connect()?;
        stream
            .write_all(request_bytes)
            .and_then(|()| stream.flush())
            .map_err(|_| {
                RemoteHttpClientError::new("HTTP_WRITE_FAILED", "HTTP request could not be sent")
            })?;
        stream.shutdown(Shutdown::Write).map_err(|_| {
            RemoteHttpClientError::new("HTTP_WRITE_FAILED", "HTTP request could not be sent")
        })?;
        read_http_reply(&mut stream, self.max_response_bytes)
    }
}

/// Reads one credential by zero-based entry index from the strict server
/// credential-file shape. This is useful for validation clients exercising
/// rotation or multiple principals without placing a secret on the command
/// line.
pub fn load_credential_file_entry(
    path: &Path,
    entry_index: usize,
) -> Result<String, RemoteHttpClientError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        RemoteHttpClientError::new(
            "CREDENTIAL_SOURCE_UNAVAILABLE",
            "credential source is unavailable",
        )
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 1024 * 1024 {
        return Err(RemoteHttpClientError::new(
            "CREDENTIAL_SOURCE_INVALID",
            "credential source is invalid",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(RemoteHttpClientError::new(
                "CREDENTIAL_SOURCE_INVALID",
                "credential source permissions are too broad",
            ));
        }
    }
    let bytes = fs::read(path).map_err(|_| {
        RemoteHttpClientError::new(
            "CREDENTIAL_SOURCE_UNAVAILABLE",
            "credential source is unavailable",
        )
    })?;
    let file: CredentialFile = serde_json::from_slice(&bytes).map_err(|_| {
        RemoteHttpClientError::new("CREDENTIAL_SOURCE_INVALID", "credential source is invalid")
    })?;
    let entry = file
        .credentials
        .into_iter()
        .nth(entry_index)
        .ok_or_else(|| {
            RemoteHttpClientError::new("CREDENTIAL_SOURCE_INVALID", "credential source is invalid")
        })?;
    if entry.principal_id.trim().is_empty()
        || entry.agent_ids.is_empty()
        || entry.agent_ids.iter().any(|agent| agent.trim().is_empty())
    {
        return Err(RemoteHttpClientError::new(
            "CREDENTIAL_SOURCE_INVALID",
            "credential source is invalid",
        ));
    }
    let credential = entry.credential;
    if credential.trim().is_empty() || credential.chars().any(char::is_whitespace) {
        return Err(RemoteHttpClientError::new(
            "CREDENTIAL_SOURCE_INVALID",
            "credential source is invalid",
        ));
    }
    Ok(credential)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialFile {
    credentials: Vec<CredentialFileEntry>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CredentialFileEntry {
    credential: String,
    #[allow(dead_code)]
    principal_id: String,
    #[allow(dead_code)]
    agent_ids: Vec<String>,
    #[allow(dead_code)]
    expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug)]
struct ParsedEndpoint {
    addr: SocketAddr,
    host_header: String,
}

impl ParsedEndpoint {
    fn parse(value: &str) -> Result<Self, RemoteHttpClientError> {
        let value = value.trim();
        let authority = if let Some(value) = value.strip_prefix("http://") {
            value
        } else if value.starts_with("https://") {
            return Err(RemoteHttpClientError::new(
                "TLS_NOT_SUPPORTED",
                "HTTPS is not supported by this client; use external TLS termination",
            ));
        } else {
            value
        };
        if authority.is_empty()
            || authority.contains('/')
            || authority.contains('?')
            || authority
                .chars()
                .any(|character| character.is_ascii_control())
        {
            return Err(RemoteHttpClientError::new(
                "INVALID_ENDPOINT",
                "HTTP endpoint is invalid",
            ));
        }
        let mut addresses = authority.to_socket_addrs().map_err(|_| {
            RemoteHttpClientError::new("INVALID_ENDPOINT", "HTTP endpoint is invalid")
        })?;
        let addr = addresses.next().ok_or_else(|| {
            RemoteHttpClientError::new("INVALID_ENDPOINT", "HTTP endpoint is invalid")
        })?;
        Ok(Self {
            addr,
            host_header: authority.to_owned(),
        })
    }
}

fn build_json_request(
    endpoint: &ParsedEndpoint,
    credential: &str,
    body: &[u8],
) -> Result<Vec<u8>, RemoteHttpClientError> {
    if body.len() > DEFAULT_REMOTE_HTTP_MAX_REQUEST_BYTES {
        return Err(RemoteHttpClientError::new(
            "REQUEST_TOO_LARGE",
            "HTTP request body is too large",
        ));
    }
    let content_length = body.len().to_string();
    let mut request = Vec::with_capacity(body.len() + 256);
    request.extend_from_slice(b"POST ");
    request.extend_from_slice(HTTP_PROTOCOL_PATH.as_bytes());
    request.extend_from_slice(b" HTTP/1.1\r\nHost: ");
    request.extend_from_slice(endpoint.host_header.as_bytes());
    request.extend_from_slice(b"\r\nAuthorization: Bearer ");
    request.extend_from_slice(credential.as_bytes());
    request.extend_from_slice(b"\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: ");
    request.extend_from_slice(content_length.as_bytes());
    request.extend_from_slice(b"\r\n\r\n");
    request.extend_from_slice(body);
    Ok(request)
}

fn validate_protocol_reply(
    reply: RemoteHttpReply,
    request_id: &str,
) -> Result<RemoteHttpReply, RemoteHttpClientError> {
    if let Some(response) = reply.response.as_ref() {
        if response.protocol_version != EXTERNAL_AGENT_PROTOCOL_VERSION {
            return Err(RemoteHttpClientError::new(
                "PROTOCOL_RESPONSE_INVALID",
                "HTTP response protocol version is invalid",
            )
            .with_http_status(reply.http_status));
        }
        if response.request_id != request_id {
            return Err(RemoteHttpClientError::new(
                "PROTOCOL_RESPONSE_INVALID",
                "HTTP response request id does not match",
            )
            .with_http_status(reply.http_status));
        }
        return Ok(reply);
    }
    if reply.http_status < 400 {
        return Err(RemoteHttpClientError::new(
            "PROTOCOL_RESPONSE_INVALID",
            "HTTP response is not a valid protocol response",
        )
        .with_http_status(reply.http_status));
    }
    // Non-2xx transport errors intentionally remain usable to callers. They
    // have no ProtocolResponse envelope and are represented by `body`.
    Ok(reply)
}

fn read_http_reply(
    stream: &mut TcpStream,
    max_response_bytes: usize,
) -> Result<RemoteHttpReply, RemoteHttpClientError> {
    let mut bytes = Vec::new();
    let header_end = read_headers(stream, &mut bytes)?;
    let headers = &bytes[..header_end];
    let status = parse_status(headers)?;
    let body_start = header_end + 4;
    match response_content_length(headers).map_err(|error| error.with_http_status(status))? {
        Some(content_length) => {
            if content_length > max_response_bytes {
                return Err(RemoteHttpClientError::new(
                    "RESPONSE_TOO_LARGE",
                    "HTTP response is too large",
                )
                .with_http_status(status));
            }
            let response_end = body_start.checked_add(content_length).ok_or_else(|| {
                RemoteHttpClientError::new("RESPONSE_TOO_LARGE", "HTTP response is too large")
                    .with_http_status(status)
            })?;
            read_exact_response_body(stream, &mut bytes, response_end)
                .map_err(|error| error.with_http_status(status))?;
            if bytes.len() > response_end {
                bytes.truncate(response_end);
            }
        }
        None => read_response_until_close(stream, &mut bytes, max_response_bytes, body_start)
            .map_err(|error| error.with_http_status(status))?,
    }
    let body = &bytes[body_start..];
    if body.len() > max_response_bytes {
        return Err(
            RemoteHttpClientError::new("RESPONSE_TOO_LARGE", "HTTP response is too large")
                .with_http_status(status),
        );
    }
    let body_json = serde_json::from_slice::<Value>(body).ok();
    let response = body_json
        .as_ref()
        .and_then(|value| serde_json::from_value::<ProtocolResponse>(value.clone()).ok());
    Ok(RemoteHttpReply {
        http_status: status,
        body: body_json,
        response,
    })
}

fn read_headers(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
) -> Result<usize, RemoteHttpClientError> {
    loop {
        if let Some(header_end) = find_header_end(bytes) {
            if header_end > MAX_HTTP_HEADER_BYTES {
                return Err(RemoteHttpClientError::new(
                    "INVALID_HTTP_RESPONSE",
                    "HTTP response headers are too large",
                ));
            }
            return Ok(header_end);
        }
        if bytes.len() > MAX_HTTP_HEADER_BYTES + 3 {
            return Err(RemoteHttpClientError::new(
                "INVALID_HTTP_RESPONSE",
                "HTTP response headers are too large",
            ));
        }
        read_more(stream, bytes)?;
    }
}

fn read_exact_response_body(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
    response_end: usize,
) -> Result<(), RemoteHttpClientError> {
    while bytes.len() < response_end {
        read_more(stream, bytes)?;
    }
    Ok(())
}

fn read_response_until_close(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
    max_response_bytes: usize,
    body_start: usize,
) -> Result<(), RemoteHttpClientError> {
    loop {
        if bytes.len().saturating_sub(body_start) > max_response_bytes {
            return Err(RemoteHttpClientError::new(
                "RESPONSE_TOO_LARGE",
                "HTTP response is too large",
            ));
        }
        if !read_more_or_eof(stream, bytes)? {
            return Ok(());
        }
    }
}

fn read_more(stream: &mut TcpStream, bytes: &mut Vec<u8>) -> Result<(), RemoteHttpClientError> {
    if read_more_or_eof(stream, bytes)? {
        Ok(())
    } else {
        Err(RemoteHttpClientError::new(
            "INVALID_HTTP_RESPONSE",
            "HTTP response ended unexpectedly",
        ))
    }
}

fn read_more_or_eof(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
) -> Result<bool, RemoteHttpClientError> {
    let mut chunk = [0_u8; 8192];
    let read = stream.read(&mut chunk).map_err(|_| {
        RemoteHttpClientError::new("HTTP_READ_FAILED", "HTTP response could not be read")
    })?;
    if read == 0 {
        return Ok(false);
    }
    bytes.extend_from_slice(&chunk[..read]);
    Ok(true)
}

fn response_content_length(headers: &[u8]) -> Result<Option<usize>, RemoteHttpClientError> {
    let text = std::str::from_utf8(headers).map_err(|_| {
        RemoteHttpClientError::new("INVALID_HTTP_RESPONSE", "HTTP response headers are invalid")
    })?;
    let mut length = None;
    for line in text.split("\r\n").skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            return Err(RemoteHttpClientError::new(
                "INVALID_HTTP_RESPONSE",
                "HTTP response headers are invalid",
            ));
        };
        if !name.trim().eq_ignore_ascii_case("Content-Length") {
            continue;
        }
        let parsed = value.trim().parse::<usize>().map_err(|_| {
            RemoteHttpClientError::new(
                "INVALID_HTTP_RESPONSE",
                "HTTP response content length is invalid",
            )
        })?;
        if length.replace(parsed).is_some() {
            return Err(RemoteHttpClientError::new(
                "INVALID_HTTP_RESPONSE",
                "HTTP response content length is ambiguous",
            ));
        }
    }
    Ok(length)
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_status(headers: &[u8]) -> Result<u16, RemoteHttpClientError> {
    let line_end = headers
        .windows(2)
        .position(|window| window == b"\r\n")
        .unwrap_or(headers.len());
    let line = std::str::from_utf8(&headers[..line_end]).map_err(|_| {
        RemoteHttpClientError::new("INVALID_HTTP_RESPONSE", "HTTP response status is invalid")
    })?;
    let mut fields = line.split_ascii_whitespace();
    let version = fields.next();
    let status = fields.next();
    if version != Some("HTTP/1.1") && version != Some("HTTP/1.0") {
        return Err(RemoteHttpClientError::new(
            "INVALID_HTTP_RESPONSE",
            "HTTP response status is invalid",
        ));
    }
    status
        .and_then(|status| status.parse::<u16>().ok())
        .filter(|status| (100..=599).contains(status))
        .ok_or_else(|| {
            RemoteHttpClientError::new("INVALID_HTTP_RESPONSE", "HTTP response status is invalid")
        })
}

impl From<io::Error> for RemoteHttpClientError {
    fn from(_: io::Error) -> Self {
        Self::new("HTTP_IO_FAILED", "HTTP transport failed")
    }
}
