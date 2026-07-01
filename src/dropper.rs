// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Dropping of stored objects, lazily offloaded to a background worker.

use crate::background_worker::BackgroundWorker;
use crate::object::Object;

/// Drop effort above which a lazily-dropped object is freed on the background
/// worker rather than inline.
const DROP_EFFORT_THRESHOLD: usize = 64;

/// Drops objects, offloading large ones to a background worker so freeing them
/// never stalls the event loop.
#[derive(Clone)]
pub struct Dropper {
    /// The worker that runs the offloaded drops.
    worker: BackgroundWorker,
}

impl Dropper {
    /// Creates a dropper, spawning its background worker.
    pub fn new() -> Self {
        Self {
            worker: BackgroundWorker::new(),
        }
    }

    /// Drops `object`. When `lazy` and the object is large enough that freeing it
    /// inline would stall the event loop, the drop is offloaded to the background
    /// worker, otherwise it happens inline.
    pub fn drop_object(&self, object: Object, lazy: bool) {
        if lazy && object.drop_effort() > DROP_EFFORT_THRESHOLD {
            self.worker.offload(move || drop(object));
        } else {
            drop(object);
        }
    }
}
