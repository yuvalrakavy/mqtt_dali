//! One correlated transaction owner per HAT. Unsolicited traffic remains public.
use super::monitor::TransportError;
use super::wire::Version;

mod engine;
pub use engine::Transactions;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RepeatPolicy {
    #[default]
    Unverified,
    OneTerminal,
    TwoTerminals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Uninitialized,
    Ready,
    Writing,
    Resynchronizing,
    Awaiting { remaining: u8 },
    WaitingLateReply { remaining: u8 },
    ResetRequired,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusReply {
    Value(u8),
    NoResponse,
    ReceiveCollision(Option<u8>),
    TransmitCollision(Option<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// A measured host-side quiet boundary; requires explicit initialization.
    Synchronized,
    Version(Version),
    Status {
        status: u8,
        channels: u8,
    },
    Dali(Vec<BusReply>),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransactionError {
    #[error("another HAT transaction is queued or active")]
    Busy,
    #[error("initialize the HAT before submitting commands")]
    NeedsInitialization,
    #[error("HAT response ordering is uncertain; initialization/reset is required")]
    Desynchronized,
    #[error("HAT transaction exceeded its total deadline")]
    Deadline,
    #[error("HAT transaction was canceled")]
    Canceled,
    #[error("HAT rebooted during the transaction")]
    Reboot,
    #[error("another DALI master interfered with the transaction")]
    Interference,
    #[error("HAT receive ordering was lost")]
    StreamLoss,
    #[error("completion for this command mode has not been verified")]
    UnsupportedMode,
    #[error("HAT transaction service is closed")]
    Closed,
    #[error(transparent)]
    Transport(#[from] TransportError),
}

#[cfg(test)]
mod tests;
