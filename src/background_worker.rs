// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! Background job execution.

use std::{
    sync::mpsc::{self, Sender},
    thread,
};

/// A unit of work to run on the worker thread.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// A worker that runs jobs on a background thread.
#[derive(Clone)]
pub struct BackgroundWorker {
    /// Channel to the worker thread.
    tx: Sender<Job>,
}

impl BackgroundWorker {
    /// Creates a worker, spawning its background thread.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        thread::spawn(move || {
            for job in rx {
                job();
            }
        });
        Self { tx }
    }

    /// Hands `job` to the worker thread to run.
    pub fn offload(&self, job: impl FnOnce() + Send + 'static) {
        let _ = self.tx.send(Box::new(job));
    }
}
