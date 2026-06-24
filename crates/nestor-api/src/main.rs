use nestor_api::{route_manifest_text, serve};
use nestor_ops::RuntimeConfig;
use std::io::IsTerminal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manifest_requested = std::env::args().any(|arg| arg == "manifest");
    let serve_requested = std::env::args().any(|arg| arg == "serve")
        || std::env::var("NESTOR_API_SERVE").is_ok_and(|value| value == "1")
        || (!manifest_requested && std::io::stdout().is_terminal());

    if serve_requested {
        serve(RuntimeConfig::from_env()?).await
    } else {
        println!("{}", route_manifest_text());
        Ok(())
    }
}
