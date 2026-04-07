/// Container engine detection
#[derive(Debug)]
pub enum Engine {
    Docker,
    Podman,
    None,
}

pub fn detect_engine() -> Engine {
    // Basic detection for Podman / Docker (currently placeholder)
    Engine::Podman
}
