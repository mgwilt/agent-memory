#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricSpec {
    pub name: &'static str,
    pub help: &'static str,
}

pub fn service_metrics() -> Vec<MetricSpec> {
    vec![
        MetricSpec {
            name: "actr_memory_retrieve_latency_ms",
            help: "User-visible retrieval latency in milliseconds.",
        },
        MetricSpec {
            name: "actr_memory_candidates_examined",
            help: "Number of Memgraph candidates scored in Rust.",
        },
        MetricSpec {
            name: "actr_memory_activation_compute_ms",
            help: "Time spent computing activation scores.",
        },
        MetricSpec {
            name: "actr_memory_retrieval_hits_total",
            help: "Successful retrieval count.",
        },
        MetricSpec {
            name: "actr_memory_retrieval_misses_total",
            help: "Threshold miss or empty-candidate retrieval count.",
        },
        MetricSpec {
            name: "actr_memory_session_lock_contention_total",
            help: "Count of requests that waited on per-agent session serialization.",
        },
        MetricSpec {
            name: "actr_memory_write_conflicts_total",
            help: "Memgraph transaction conflict count.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_reported_candidate_metric() {
        assert!(
            service_metrics()
                .iter()
                .any(|metric| metric.name == "actr_memory_candidates_examined")
        );
    }
}
