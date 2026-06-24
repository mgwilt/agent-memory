use std::{
    io::{Read, Write},
    net::TcpStream,
};

use serde::{Serialize, de::DeserializeOwned};

use crate::{ApiClientConfig, ClientError};

#[derive(Debug, Clone)]
pub struct HttpTransport {
    config: ApiClientConfig,
}

#[derive(Debug, Clone)]
pub struct RawResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedUrl {
    host: String,
    port: u16,
    base_path: String,
}

impl HttpTransport {
    pub fn new(config: ApiClientConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &ApiClientConfig {
        &self.config
    }

    pub async fn get_json<T>(&self, path: &str) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
    {
        let response = self.request("GET", path, None)?;
        decode_json_response(response)
    }

    pub async fn delete_json<T>(&self, path: &str) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
    {
        let response = self.request("DELETE", path, None)?;
        decode_json_response(response)
    }

    pub async fn put_json<T, B>(&self, path: &str, body: &B) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let body = serde_json::to_vec(body)?;
        let response = self.request("PUT", path, Some(body))?;
        decode_json_response(response)
    }

    pub async fn post_json<T, B>(&self, path: &str, body: &B) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let body = serde_json::to_vec(body)?;
        let response = self.request("POST", path, Some(body))?;
        decode_json_response(response)
    }

    pub async fn patch_json<T, B>(&self, path: &str, body: &B) -> Result<T, ClientError>
    where
        T: DeserializeOwned,
        B: Serialize,
    {
        let body = serde_json::to_vec(body)?;
        let response = self.request("PATCH", path, Some(body))?;
        decode_json_response(response)
    }

    pub async fn get_text(&self, path: &str) -> Result<String, ClientError> {
        let response = self.request("GET", path, None)?;
        if !(200..300).contains(&response.status) {
            return Err(decode_problem(response.body));
        }
        String::from_utf8(response.body)
            .map_err(|err| ClientError::InvalidResponse(format!("body is not UTF-8: {err}")))
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<RawResponse, ClientError> {
        let parsed = parse_http_url(&self.config.api_url)?;
        let request_path = format!("{}{}", parsed.base_path, path);
        let address = format!("{}:{}", parsed.host, parsed.port);
        let mut stream =
            TcpStream::connect(&address).map_err(|err| ClientError::Transport(err.to_string()))?;
        stream
            .set_read_timeout(Some(self.config.timeout))
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        stream
            .set_write_timeout(Some(self.config.timeout))
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        let body = body.unwrap_or_default();
        let request = format!(
            "{method} {request_path} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            parsed.host,
            body.len()
        );
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.write_all(&body))
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|err| ClientError::Transport(err.to_string()))?;
        parse_response(&response)
    }
}

fn decode_json_response<T>(response: RawResponse) -> Result<T, ClientError>
where
    T: DeserializeOwned,
{
    if !(200..300).contains(&response.status) {
        return Err(decode_problem(response.body));
    }
    serde_json::from_slice(&response.body)
        .map_err(|err| ClientError::InvalidResponse(format!("failed to decode JSON: {err}")))
}

fn decode_problem(body: Vec<u8>) -> ClientError {
    match serde_json::from_slice(&body) {
        Ok(problem) => ClientError::Api(problem),
        Err(err) => ClientError::InvalidResponse(format!("failed to decode API problem: {err}")),
    }
}

fn parse_http_url(url: &str) -> Result<ParsedUrl, ClientError> {
    let without_scheme = url
        .strip_prefix("http://")
        .ok_or_else(|| ClientError::InvalidUrl("only http:// URLs are supported".to_string()))?;
    let (authority, path) = without_scheme
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{path}")))
        .unwrap_or((without_scheme, String::new()));
    if authority.trim().is_empty() {
        return Err(ClientError::InvalidUrl("missing host".to_string()));
    }
    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        let parsed_port = port
            .parse::<u16>()
            .map_err(|_| ClientError::InvalidUrl("port must be an integer".to_string()))?;
        (host.to_string(), parsed_port)
    } else {
        (authority.to_string(), 80)
    };
    let base_path = path.trim_end_matches('/').to_string();
    Ok(ParsedUrl {
        host,
        port,
        base_path,
    })
}

fn parse_response(response: &[u8]) -> Result<RawResponse, ClientError> {
    let marker = b"\r\n\r\n";
    let split_at = response
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or_else(|| ClientError::InvalidResponse("missing header terminator".to_string()))?;
    let header = String::from_utf8(response[..split_at].to_vec())
        .map_err(|err| ClientError::InvalidResponse(format!("headers are not UTF-8: {err}")))?;
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| ClientError::InvalidResponse("missing status code".to_string()))?;
    Ok(RawResponse {
        status,
        body: response[(split_at + marker.len())..].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_url_with_port() {
        let parsed = parse_http_url("http://127.0.0.1:8080").unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 8080);
        assert_eq!(parsed.base_path, "");
    }

    #[test]
    fn parses_basic_response() {
        let response = parse_response(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}")
            .unwrap_or_else(|err| {
                panic!("{err}");
            });
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"{}");
    }
}
