#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    Counter,
    Gauge,
}

impl MetricKind {
    pub fn prometheus_type(self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gauge => "gauge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricSpec {
    pub name: &'static str,
    pub help: &'static str,
    pub kind: MetricKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricSample {
    pub name: &'static str,
    pub value: f64,
}

pub fn service_metrics() -> Vec<MetricSpec> {
    vec![
        MetricSpec {
            name: "nestor_memory_retrieve_latency_ms",
            help: "Last observed retrieval request latency in milliseconds.",
            kind: MetricKind::Gauge,
        },
        MetricSpec {
            name: "nestor_memory_candidates_examined",
            help: "Last observed number of candidates scored in Rust.",
            kind: MetricKind::Gauge,
        },
        MetricSpec {
            name: "nestor_memory_activation_compute_ms",
            help: "Last observed time spent computing activation scores in milliseconds.",
            kind: MetricKind::Gauge,
        },
        MetricSpec {
            name: "nestor_memory_retrieval_hits_total",
            help: "Successful retrieval count.",
            kind: MetricKind::Counter,
        },
        MetricSpec {
            name: "nestor_memory_retrieval_misses_total",
            help: "Threshold miss or empty-candidate retrieval count.",
            kind: MetricKind::Counter,
        },
        MetricSpec {
            name: "nestor_memory_session_lock_contention_total",
            help: "Count of requests that waited on per-agent session serialization.",
            kind: MetricKind::Counter,
        },
        MetricSpec {
            name: "nestor_memory_write_conflicts_total",
            help: "Memgraph transaction conflict count.",
            kind: MetricKind::Counter,
        },
    ]
}

pub fn render_prometheus_metrics(samples: &[MetricSample]) -> String {
    let mut output = String::new();
    for spec in service_metrics() {
        output.push_str("# HELP ");
        output.push_str(spec.name);
        output.push(' ');
        output.push_str(spec.help);
        output.push('\n');
        output.push_str("# TYPE ");
        output.push_str(spec.name);
        output.push(' ');
        output.push_str(spec.kind.prometheus_type());
        output.push('\n');
        output.push_str(spec.name);
        output.push(' ');
        output.push_str(&format_metric_value(sample_value(spec.name, samples)));
        output.push('\n');
    }
    output
}

fn sample_value(name: &str, samples: &[MetricSample]) -> f64 {
    samples
        .iter()
        .find(|sample| sample.name == name)
        .map_or(0.0, |sample| sample.value)
}

fn format_metric_value(value: f64) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_required_service_metrics() {
        let metrics = service_metrics();
        for name in [
            "nestor_memory_retrieve_latency_ms",
            "nestor_memory_candidates_examined",
            "nestor_memory_activation_compute_ms",
            "nestor_memory_retrieval_hits_total",
            "nestor_memory_retrieval_misses_total",
            "nestor_memory_session_lock_contention_total",
            "nestor_memory_write_conflicts_total",
        ] {
            assert!(metrics.iter().any(|metric| metric.name == name));
        }
    }

    #[test]
    fn renders_prometheus_text_with_types_and_defaults() {
        let output = render_prometheus_metrics(&[
            MetricSample {
                name: "nestor_memory_retrieval_hits_total",
                value: 2.0,
            },
            MetricSample {
                name: "nestor_memory_retrieve_latency_ms",
                value: 3.5,
            },
        ]);

        assert!(output.contains("# TYPE nestor_memory_retrieval_hits_total counter"));
        assert!(output.contains("nestor_memory_retrieval_hits_total 2"));
        assert!(output.contains("# TYPE nestor_memory_retrieve_latency_ms gauge"));
        assert!(output.contains("nestor_memory_retrieve_latency_ms 3.5"));
        assert!(output.contains("nestor_memory_candidates_examined 0"));
    }

    #[test]
    fn non_finite_samples_are_rendered_as_zero() {
        let output = render_prometheus_metrics(&[MetricSample {
            name: "nestor_memory_activation_compute_ms",
            value: f64::NAN,
        }]);

        assert!(output.contains("nestor_memory_activation_compute_ms 0"));
    }
}
