// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

//! Primitive building blocks of mpsc channel signalling implementation

use crate::error::Error;
use crate::signalling::common::mpsc::endpoint::Measure;
use core::fmt;
use core::sync::atomic::AtomicU64;
use core::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::println;
use std::sync::mpsc::{RecvTimeoutError, SendError};
use std::sync::{mpsc, Mutex, OnceLock};

const MPSC_CHANNEL_BOUND: usize = 128;

// Static global container: key -> KeyData
static GLOBAL_LOGGER: OnceLock<ShutdownLogger> = OnceLock::new();

static XDF: AtomicU64 = AtomicU64::new(0);

pub fn dropppp() {
    ShutdownLogger::get().print();
}
struct KeyData {
    timestamps: VecDeque<(std::time::Instant, Option<(std::time::Instant, u64)>)>,
}

struct ShutdownLogger {
    map: Mutex<HashMap<u64, KeyData>>,
}

impl ShutdownLogger {
    fn get() -> &'static ShutdownLogger {
        GLOBAL_LOGGER.get_or_init(|| ShutdownLogger {
            map: Mutex::new(HashMap::new()),
        })
    }

    fn insert_timestamp_notifier(&self, key: u64) {
        let now = std::time::Instant::now();

        let mut map = self.map.lock().unwrap();
        let entry = map.entry(key).or_insert_with(|| KeyData {
            timestamps: VecDeque::new(),
        });

        entry.timestamps.push_back((now, None));
    }

    fn insert_timestamp_listener(&self, key: u64) {
        let mut map = self.map.lock().unwrap();
        let e = map.get_mut(&key);

        if e.is_none() {
            panic!("No entry found for key: {}", key);
            return;
        }

        let entry = e.unwrap();

        let val = entry.timestamps.iter_mut().last().unwrap();

        val.1 = Some((
            std::time::Instant::now(),
            std::time::Instant::elapsed(&val.0).as_micros() as u64,
        ));
    }

    fn print(&self) {
        let map = self.map.lock().unwrap();
        println!("\n=== Shutdown: Final Timestamp Dump ===");
        for (key, data) in map.iter() {
            println!("Key: {:?}", key);
            for ts in &data.timestamps {
                println!("{:?}", ts);
            }
            println!("---");
        }
        println!("=== End ===");
    }
}

pub(crate) fn channel<T: fmt::Debug + Measure>() -> (Sender<T>, Receiver<T>) {
    let id = XDF.fetch_add(1, core::sync::atomic::Ordering::SeqCst);

    let (mpsc_sender, mpsc_receiver) = mpsc::sync_channel(MPSC_CHANNEL_BOUND);
    let sender = Sender::new(mpsc_sender, id);
    let receiver = Receiver::new(mpsc_receiver, id);
    (sender, receiver)
}

pub(crate) struct Receiver<T: fmt::Debug + Measure + 'static> {
    id: u64,
    pub(crate) receiver: mpsc::Receiver<T>,
}

impl<T: fmt::Debug + Measure + 'static> Receiver<T> {
    pub(crate) fn new(receiver: mpsc::Receiver<T>, id: u64) -> Self {
        Self { receiver, id }
    }

    pub fn receive(&mut self, timeout: Duration) -> Result<Option<T>, Error> {
        match self.receiver.recv_timeout(timeout) {
            Ok(v) => {
                if v.is_step() {
                    ShutdownLogger::get().insert_timestamp_listener(self.id);
                }

                Ok(Some(v))
            }
            Err(err) => match err {
                RecvTimeoutError::Timeout => Ok(None),
                _ => Err(Error::Channel("channel closed")),
            },
        }
    }
}

#[derive(Clone)]
pub(crate) struct Sender<T: fmt::Debug + Measure + 'static> {
    id: u64,
    pub(crate) sender: mpsc::SyncSender<T>,
}

impl<T: fmt::Debug + Measure + 'static> Sender<T> {
    pub(crate) fn new(sender: mpsc::SyncSender<T>, id: u64) -> Self {
        Self { sender, id }
    }

    pub fn send_stub(&self, t: T) -> Result<(), SendError<T>> {
        if t.is_step() {
            ShutdownLogger::get().insert_timestamp_notifier(self.id);
        }

        self.sender.send(t)
    }

    pub fn send(&mut self, t: T) -> Result<(), Error> {
        self.sender
            .send(t)
            .map_err(|_| Error::Channel("channel closed"))?;
        Ok(())
    }
}
