pub mod capture;
pub mod fft;
pub mod fixtures;
pub mod mpris;
pub mod policy;
pub mod presence;
pub mod protocol;
pub mod socket;

pub struct CommandRequest {
    pub command: protocol::Command,
    pub reply: tokio::sync::oneshot::Sender<Result<(), protocol::Failure>>,
}
