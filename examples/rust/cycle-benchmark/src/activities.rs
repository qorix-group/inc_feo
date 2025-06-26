// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use feo::ids::ActivityId;
use feo::{activity::Activity, orch_adapter::types::ActivityAdapterTrait};
use feo_tracing::{instrument, tracing};
use tracing::{span, Level};

/// This is a dummy activity that does nothing.
#[derive(Debug)]
pub struct DummyActivity {
    /// ID of the activity
    activity_id: ActivityId,
    /// ID as string (only used for tracing)
    _id_str: String,
    dur: Duration,
}

impl DummyActivity {
    pub fn build(activity_id: ActivityId) -> Box<dyn Activity> {
        Box::new(Self {
            activity_id,
            _id_str: u64::from(activity_id).to_string(),
            dur: Duration::ZERO,
        })
    }

    pub fn build_orch(activity_id: ActivityId, dur: Duration) -> Self {
        Self {
            activity_id,
            _id_str: u64::from(activity_id).to_string(),
            dur,
        }
    }
}

impl Activity for DummyActivity {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    #[instrument(name = "Activity startup")]
    fn startup(&mut self) {}

    #[instrument(name = "Activity step")]
    fn step(&mut self) {
        tracing::event!(tracing::Level::TRACE, id = self._id_str);

        if self.id() == ActivityId::from(1) {
            // println!("Current time: {:?}", std::time::Instant::now());
        }

        // std::thread::sleep(self.dur);
    }

    #[instrument(name = "Activity shutdown")]
    fn shutdown(&mut self) {}
}

unsafe impl Send for DummyActivity {}

impl ActivityAdapterTrait for DummyActivity {
    type T = Self;

    fn step_runtime(
        instance: std::sync::Arc<std::sync::Mutex<Self::T>>,
    ) -> impl std::future::Future<Output = orchestration::actions::invoke::InvokeResult> + Send
    {
        async move {
            instance.lock().unwrap().step();
            Ok(())
        }
    }

    fn start(&mut self) -> orchestration::actions::invoke::InvokeResult {
        self.startup();
        Ok(())
    }

    fn stop(&mut self) -> orchestration::actions::invoke::InvokeResult {
        self.shutdown();
        Ok(())
    }

    fn get_act_id(&self) -> ActivityId {
        self.activity_id
    }
}
