// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use async_runtime::runtime::runtime::AsyncRuntimeBuilder;
use async_runtime::scheduler::execution_engine::ExecutionEngineBuilder;
use feo::prelude::*;
use feo_log::info;
use feo_mini_adas::activities::components::{EnvironmentRenderer, NeuralNet, SECONDARY1_NAME};
use feo_mini_adas::activities::runtime_adapters::{activity_into_invokes, LocalFeoAgent};
use feo_mini_adas::config::*;
use foundation::threading::thread_wait_barrier::*;
use logging_tracing::prelude::*;
use orchestration::actions::event::Event;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// This agent's ID
const AGENT_ID: AgentId = AgentId::new(101);

fn main() {
    let mut logger = TracingLibraryBuilder::new()
        .global_log_level(Level::DEBUG)
        .enable_tracing(TraceScope::SystemScope)
        .enable_logging(false)
        .build();

    logger.init_log_trace();

    info!("Starting agent {AGENT_ID}");

    let mut runtime = AsyncRuntimeBuilder::new()
        .with_engine(
            ExecutionEngineBuilder::new()
                .task_queue_size(256)
                .workers(2),
        )
        .build()
        .unwrap();

    Event::get_instance()
        .lock()
        .unwrap()
        .create_polling_thread();

    // Since runtime `enter_engine` is now not blocking, we do it manually here.
    let waiter = Arc::new(ThreadWaitBarrier::new(1));
    let notifier = waiter.get_notifier().unwrap();

    runtime
        .enter_engine(async {
            let neural_net_act = Arc::new(Mutex::new(NeuralNet::build_val(
                3.into(),
                TOPIC_CAMERA_FRONT,
                TOPIC_RADAR_FRONT,
                TOPIC_INFERRED_SCENE,
            )));

            let environ_renderer_act = Arc::new(Mutex::new(EnvironmentRenderer::build(
                4.into(),
                TOPIC_INFERRED_SCENE,
            )));

            let acts = vec![
                activity_into_invokes(&neural_net_act),
                activity_into_invokes(&environ_renderer_act),
            ];

            let mut agent = LocalFeoAgent::new(acts, SECONDARY1_NAME);
            let mut program = agent.create_program();

            program.run().await;
            info!("Finished");
            notifier.ready();
        })
        .unwrap_or_default();

    waiter
        .wait_for_all(Duration::new(2000, 0))
        .unwrap_or_default();
}
