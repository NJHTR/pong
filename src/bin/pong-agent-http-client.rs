//! Independent-process validation client for the HTTP External Agent Protocol.
//!
//! Normal mode reads one ProtocolRequest JSON object per stdin line and emits
//! one bounded JSON result per line. `--raw-body` and `--raw-http` are explicit
//! conformance modes for malformed-input/resource-boundary tests; they are not
//! alternate protocol formats.

#[path = "../remote_client.rs"]
mod remote_client;

use remote_client::{
    load_credential_file_entry, RemoteHttpClient, RemoteHttpClientConfig, RemoteHttpReply,
    DEFAULT_REMOTE_HTTP_MAX_RESPONSE_BYTES,
};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputMode {
    Protocol,
    RawBody,
    RawHttp,
}

struct Arguments {
    endpoint: String,
    credentials_file: PathBuf,
    credential_index: usize,
    mode: InputMode,
    input_file: Option<PathBuf>,
    disconnect_after_send: bool,
    timeout: Duration,
    max_response_bytes: usize,
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut endpoint = None;
    let mut credentials_file = None;
    let mut credential_index = 0;
    let mut mode = InputMode::Protocol;
    let mut input_file = None;
    let mut disconnect_after_send = false;
    let mut timeout = Duration::from_secs(10);
    let mut max_response_bytes = DEFAULT_REMOTE_HTTP_MAX_RESPONSE_BYTES;
    let mut arguments = env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--endpoint") => endpoint = Some(required_text(&mut arguments)?),
            Some("--credentials-file") => credentials_file = Some(required_path(&mut arguments)?),
            Some("--credential-index") => {
                credential_index = required_text(&mut arguments)?
                    .parse()
                    .map_err(|_| usage())?;
            }
            Some("--raw-body") => mode = select_mode(mode, InputMode::RawBody)?,
            Some("--raw-http") => mode = select_mode(mode, InputMode::RawHttp)?,
            Some("--raw-body-file") => {
                mode = select_mode(mode, InputMode::RawBody)?;
                input_file = Some(required_path(&mut arguments)?);
            }
            Some("--raw-http-file") => {
                mode = select_mode(mode, InputMode::RawHttp)?;
                input_file = Some(required_path(&mut arguments)?);
            }
            Some("--disconnect-after-send") => disconnect_after_send = true,
            Some("--timeout-ms") => {
                let millis: u64 = required_text(&mut arguments)?
                    .parse()
                    .map_err(|_| usage())?;
                if millis == 0 {
                    return Err(usage());
                }
                timeout = Duration::from_millis(millis);
            }
            Some("--max-response-bytes") => {
                max_response_bytes = required_text(&mut arguments)?
                    .parse()
                    .map_err(|_| usage())?;
                if max_response_bytes == 0 {
                    return Err(usage());
                }
            }
            _ => return Err(usage()),
        }
    }
    if input_file.is_some() && mode == InputMode::Protocol {
        return Err(usage());
    }
    Ok(Arguments {
        endpoint: endpoint.ok_or_else(usage)?,
        credentials_file: credentials_file.ok_or_else(usage)?,
        credential_index,
        mode,
        input_file,
        disconnect_after_send,
        timeout,
        max_response_bytes,
    })
}

fn select_mode(current: InputMode, requested: InputMode) -> Result<InputMode, String> {
    if current != InputMode::Protocol && current != requested {
        return Err(usage());
    }
    Ok(requested)
}

fn required_text(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<String, String> {
    arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(usage)
}

fn required_path(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<PathBuf, String> {
    arguments.next().map(PathBuf::from).ok_or_else(usage)
}

fn usage() -> String {
    "usage: pong-agent-http-client --endpoint HOST:PORT --credentials-file PATH [--credential-index N] [--timeout-ms MILLISECONDS] [--max-response-bytes BYTES] [--disconnect-after-send] [--raw-body [--raw-body-file PATH] | --raw-http [--raw-http-file PATH]]".into()
}

fn run(arguments: Arguments) -> Result<(), String> {
    let credential =
        load_credential_file_entry(&arguments.credentials_file, arguments.credential_index)
            .map_err(|error| error.to_string())?;
    let client = RemoteHttpClient::from_config(RemoteHttpClientConfig {
        endpoint: arguments.endpoint,
        credential,
        timeout: arguments.timeout,
        max_response_bytes: arguments.max_response_bytes,
    })
    .map_err(|error| error.to_string())?;
    match arguments.mode {
        InputMode::Protocol => run_protocol(client, arguments.disconnect_after_send),
        InputMode::RawBody => run_raw_body(
            client,
            arguments.input_file,
            arguments.disconnect_after_send,
        ),
        InputMode::RawHttp => run_raw_http(
            client,
            arguments.input_file,
            arguments.disconnect_after_send,
        ),
    }
}

fn run_protocol(client: RemoteHttpClient, disconnect_after_send: bool) -> Result<(), String> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line.map_err(|_| "CLIENT_INPUT_FAILED: input could not be read".to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(_) => {
                write_json_line(
                    &mut stdout,
                    &json!({"http_status": Value::Null, "response": Value::Null, "body": Value::Null, "error": {"code": "INVALID_PROTOCOL_REQUEST", "message": "request is not valid protocol JSON", "retryable": false}}),
                )?;
                continue;
            }
        };
        if disconnect_after_send {
            let body = serde_json::to_vec(&parsed).map_err(|_| {
                "CLIENT_SERIALIZATION_FAILED: request is not serializable".to_string()
            })?;
            let result = client.send_json_without_reading(&body);
            write_send_result(&mut stdout, result)?;
            break;
        }
        match client.request(&parsed) {
            Ok(reply) => write_reply(&mut stdout, reply)?,
            Err(error) => write_error(&mut stdout, &error)?,
        }
    }
    stdout
        .flush()
        .map_err(|_| "CLIENT_OUTPUT_FAILED: output could not be written".into())
}

fn run_raw_body(
    client: RemoteHttpClient,
    input_file: Option<PathBuf>,
    disconnect_after_send: bool,
) -> Result<(), String> {
    let body = read_one_shot_input(input_file)?;
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    if disconnect_after_send {
        write_send_result(&mut stdout, client.send_json_without_reading(&body))?;
    } else {
        match client.send_json_body(&body) {
            Ok(reply) => write_reply(&mut stdout, reply)?,
            Err(error) => write_error(&mut stdout, &error)?,
        }
    }
    stdout
        .flush()
        .map_err(|_| "CLIENT_OUTPUT_FAILED: output could not be written".into())
}

fn run_raw_http(
    client: RemoteHttpClient,
    input_file: Option<PathBuf>,
    disconnect_after_send: bool,
) -> Result<(), String> {
    let request = read_one_shot_input(input_file)?;
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    if disconnect_after_send {
        write_send_result(&mut stdout, client.send_without_reading(&request))?;
    } else {
        match client.send_raw_http(&request) {
            Ok(reply) => write_reply(&mut stdout, reply)?,
            Err(error) => write_error(&mut stdout, &error)?,
        }
    }
    stdout
        .flush()
        .map_err(|_| "CLIENT_OUTPUT_FAILED: output could not be written".into())
}

fn read_one_shot_input(input_file: Option<PathBuf>) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    if let Some(path) = input_file {
        fs::File::open(path)
            .map_err(|_| "CLIENT_INPUT_FAILED: input file could not be read".to_string())?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "CLIENT_INPUT_FAILED: input file could not be read".to_string())?;
    } else {
        io::stdin()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "CLIENT_INPUT_FAILED: input could not be read".to_string())?;
    }
    if bytes.len() > 1024 * 1024 {
        return Err("CLIENT_INPUT_FAILED: input body is too large".into());
    }
    Ok(bytes)
}

fn write_reply(output: &mut impl Write, reply: RemoteHttpReply) -> Result<(), String> {
    let value = json!({
        "http_status": reply.http_status,
        "response": reply.response,
        "body": reply.body,
        "error": Value::Null,
    });
    write_json_line(output, &value)
}

fn write_send_result(
    output: &mut impl Write,
    result: Result<(), remote_client::RemoteHttpClientError>,
) -> Result<(), String> {
    let value = match result {
        Ok(()) => {
            json!({"http_status": Value::Null, "sent": true, "response": Value::Null, "body": Value::Null, "error": Value::Null})
        }
        Err(error) => {
            json!({"http_status": error.http_status(), "sent": false, "response": Value::Null, "body": Value::Null, "error": {"code": error.code(), "message": error.message(), "retryable": false}})
        }
    };
    write_json_line(output, &value)
}

fn write_error(
    output: &mut impl Write,
    error: &remote_client::RemoteHttpClientError,
) -> Result<(), String> {
    write_json_line(
        output,
        &json!({"http_status": error.http_status(), "response": Value::Null, "body": Value::Null, "error": {"code": error.code(), "message": error.message(), "retryable": false}}),
    )
}

fn write_json_line(output: &mut impl Write, value: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *output, value)
        .map_err(|_| "CLIENT_OUTPUT_FAILED: output could not be serialized".to_string())?;
    output
        .write_all(b"\n")
        .and_then(|_| output.flush())
        .map_err(|_| "CLIENT_OUTPUT_FAILED: output could not be written".to_string())
}

fn main() -> ExitCode {
    match parse_arguments().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pong-agent-http-client: {message}");
            ExitCode::from(2)
        }
    }
}
