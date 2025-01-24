//! Helper models.
//!
//! This module contains helper models useful for simulation bench assembly.
//!

use std::time::Duration;

use nexosim::model::{Context, InitializedModel, Model};

/// A ticker model.
///
/// This model self-schedules with a provided period keeping simulation alive.
pub struct Ticker {
    /// Tick period in milliseconds.
    tick: Duration,
}

impl Ticker {
    /// Creates new `Ticker` with provided tick period in milliseconds.
    pub fn new(tick: Duration) -> Self {
        Self { tick }
    }

    /// Self-scheduled function.
    pub async fn tick(&mut self) {}
}

impl Model for Ticker {
    async fn init(self, cx: &mut Context<Self>) -> InitializedModel<Self> {
        cx.schedule_periodic_event(self.tick, self.tick, Self::tick, ())
            .unwrap();
        self.into()
    }
}
