#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteSpec {
    pub method: &'static str,
    pub path: &'static str,
    pub purpose: &'static str,
}

pub fn route_manifest() -> Vec<RouteSpec> {
    vec![
        RouteSpec {
            method: "POST",
            path: "/v1/memory/chunks",
            purpose: "create or upsert chunk",
        },
        RouteSpec {
            method: "GET",
            path: "/v1/memory/chunks/{chunk_id}",
            purpose: "inspect chunk",
        },
        RouteSpec {
            method: "PATCH",
            path: "/v1/memory/chunks/{chunk_id}",
            purpose: "update chunk slots with optimistic versioning",
        },
        RouteSpec {
            method: "DELETE",
            path: "/v1/memory/chunks/{chunk_id}",
            purpose: "soft-delete chunk",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/memory/retrieve",
            purpose: "ACT-R retrieval with score breakdown",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/memory/retrieve/stream",
            purpose: "progressive retrieval diagnostics over SSE",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/memory/practice",
            purpose: "record encoding, retrieval, or rehearsal",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/memory/associate",
            purpose: "add or update spreading-activation association",
        },
        RouteSpec {
            method: "PUT",
            path: "/v1/memory/buffers/{buffer_name}",
            purpose: "set current buffer chunk",
        },
        RouteSpec {
            method: "POST",
            path: "/v1/rules/evaluate",
            purpose: "evaluate production candidates",
        },
        RouteSpec {
            method: "GET",
            path: "/healthz",
            purpose: "liveness",
        },
        RouteSpec {
            method: "GET",
            path: "/readyz",
            purpose: "readiness with Memgraph probe",
        },
        RouteSpec {
            method: "GET",
            path: "/metrics",
            purpose: "Prometheus scrape",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_manifest_contains_reported_endpoints() {
        let routes = route_manifest();

        assert!(
            routes
                .iter()
                .any(|route| route.path == "/v1/memory/retrieve")
        );
        assert!(routes.iter().any(|route| route.path == "/metrics"));
        assert!(routes.iter().any(|route| route.path == "/readyz"));
    }
}
