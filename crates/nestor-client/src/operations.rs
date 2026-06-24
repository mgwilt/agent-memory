use nestor_api::{
    AssociateRequest, AssociateResponse, BufferResponse, BufferSetRequest, ChunkPatchRequest,
    ChunkResponse, ChunkUpsertRequest, ConsolidateRequest, ConsolidateResponse, DeleteResponse,
    ForgetRequest, ForgetResponse, HealthResponse, PracticeRequest, PracticeResponse,
    RehearseRequest, RehearseResponse, RetrieveRequest, RetrieveResponse, RouteSpec,
    RuleEvaluateRequest, RuleEvaluateResponse, route_manifest,
};

use crate::{ApiClientConfig, ClientError, http::HttpTransport};

#[derive(Debug, Clone)]
pub struct NestorClient {
    http: HttpTransport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRoute {
    pub method: String,
    pub path: String,
    pub purpose: String,
}

impl From<RouteSpec> for HttpRoute {
    fn from(value: RouteSpec) -> Self {
        Self {
            method: value.method.to_string(),
            path: value.path.to_string(),
            purpose: value.purpose.to_string(),
        }
    }
}

impl NestorClient {
    pub fn new(config: ApiClientConfig) -> Self {
        Self {
            http: HttpTransport::new(config),
        }
    }

    pub fn config(&self) -> &ApiClientConfig {
        self.http.config()
    }

    pub fn manifest(&self) -> Vec<HttpRoute> {
        route_manifest().into_iter().map(HttpRoute::from).collect()
    }

    pub async fn put_chunk(
        &self,
        request: &ChunkUpsertRequest,
    ) -> Result<ChunkResponse, ClientError> {
        self.http.post_json("/v1/memory/chunks", request).await
    }

    pub async fn get_chunk(
        &self,
        agent_id: &str,
        chunk_id: &str,
    ) -> Result<ChunkResponse, ClientError> {
        self.http
            .get_json(&format!(
                "/v1/memory/chunks/{}?agent_id={}",
                encode_path(chunk_id),
                encode_query(agent_id)
            ))
            .await
    }

    pub async fn patch_chunk(
        &self,
        chunk_id: &str,
        request: &ChunkPatchRequest,
    ) -> Result<ChunkResponse, ClientError> {
        self.http
            .patch_json(
                &format!("/v1/memory/chunks/{}", encode_path(chunk_id)),
                request,
            )
            .await
    }

    pub async fn delete_chunk(
        &self,
        agent_id: &str,
        chunk_id: &str,
    ) -> Result<DeleteResponse, ClientError> {
        self.http
            .delete_json(&format!(
                "/v1/memory/chunks/{}?agent_id={}",
                encode_path(chunk_id),
                encode_query(agent_id)
            ))
            .await
    }

    pub async fn retrieve_memory(
        &self,
        request: &RetrieveRequest,
    ) -> Result<RetrieveResponse, ClientError> {
        self.http.post_json("/v1/memory/retrieve", request).await
    }

    pub async fn retrieve_memory_stream_endpoint(
        &self,
        request: &RetrieveRequest,
    ) -> Result<RetrieveResponse, ClientError> {
        self.http
            .post_json("/v1/memory/retrieve/stream", request)
            .await
    }

    pub async fn record_practice(
        &self,
        request: &PracticeRequest,
    ) -> Result<PracticeResponse, ClientError> {
        self.http.post_json("/v1/memory/practice", request).await
    }

    pub async fn rehearse_memory(
        &self,
        request: &RehearseRequest,
    ) -> Result<RehearseResponse, ClientError> {
        self.http.post_json("/v1/memory/rehearse", request).await
    }

    pub async fn consolidate_memory(
        &self,
        request: &ConsolidateRequest,
    ) -> Result<ConsolidateResponse, ClientError> {
        self.http.post_json("/v1/memory/consolidate", request).await
    }

    pub async fn forget_memory(
        &self,
        request: &ForgetRequest,
    ) -> Result<ForgetResponse, ClientError> {
        self.http.post_json("/v1/memory/forget", request).await
    }

    pub async fn upsert_association(
        &self,
        request: &AssociateRequest,
    ) -> Result<AssociateResponse, ClientError> {
        self.http.post_json("/v1/memory/associate", request).await
    }

    pub async fn set_buffer(
        &self,
        buffer_name: &str,
        request: &BufferSetRequest,
    ) -> Result<BufferResponse, ClientError> {
        self.http
            .put_json(
                &format!("/v1/memory/buffers/{}", encode_path(buffer_name)),
                request,
            )
            .await
    }

    pub async fn evaluate_rules(
        &self,
        request: &RuleEvaluateRequest,
    ) -> Result<RuleEvaluateResponse, ClientError> {
        self.http.post_json("/v1/rules/evaluate", request).await
    }

    pub async fn health(&self) -> Result<HealthResponse, ClientError> {
        self.http.get_json("/healthz").await
    }

    pub async fn ready(&self) -> Result<HealthResponse, ClientError> {
        self.http.get_json("/readyz").await
    }

    pub async fn metrics(&self) -> Result<String, ClientError> {
        self.http.get_text("/metrics").await
    }
}

fn encode_path(value: &str) -> String {
    encode_component(value)
}

fn encode_query(value: &str) -> String {
    encode_component(value)
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use crate::ApiClientConfig;

    use super::*;

    #[test]
    fn manifest_exposes_all_routes() {
        let client = NestorClient::new(ApiClientConfig::default());
        let routes = client.manifest();
        assert!(routes.iter().any(|route| route.path == "/v1/memory/chunks"));
        assert!(routes.iter().any(|route| route.path == "/metrics"));
    }

    #[test]
    fn encodes_path_and_query_components() {
        assert_eq!(encode_path("a/b c"), "a%2Fb%20c");
        assert_eq!(encode_query("agent 1"), "agent%201");
    }
}
