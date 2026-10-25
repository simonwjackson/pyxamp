pub mod capture;
pub mod config;
pub mod engine;
pub mod fft;
pub mod fixtures;
pub mod mpris;
pub mod policy;
pub mod presence;
pub mod protocol;
pub mod session;
pub mod socket;
pub mod sources;
pub mod system_volume;

pub struct CommandRequest {
    pub command: protocol::Command,
    pub reply: tokio::sync::oneshot::Sender<Result<(), protocol::Failure>>,
}
