// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Background dropping of large values.

use crate::background_worker::BackgroundWorker;
use crate::object::Object;

/// Drop effort above which an object is dropped on the background worker rather
/// than inline.
const DROP_EFFORT_THRESHOLD: usize = 64;

/// Drops objects, offloading large ones to a background worker so dropping them
/// never stalls the event loop.
#[derive(Clone)]
pub struct LazyDropper {
    /// The worker that runs the deferred drops.
    worker: BackgroundWorker,
}

impl LazyDropper {
    /// Creates a lazy dropper, spawning its background worker.
    pub fn new() -> Self {
        Self {
            worker: BackgroundWorker::new(),
        }
    }

    /// Drops `object`, offloading the drop to the background worker when it is
    /// large enough that dropping it inline would stall the event loop.
    pub fn drop(&self, object: Object) {
        if object.drop_effort() > DROP_EFFORT_THRESHOLD {
            self.worker.offload(move || drop(object));
        } else {
            drop(object);
        }
    }
}
