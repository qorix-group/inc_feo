// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use feo::prelude::*;
use feo_log::{info, LevelFilter};
use feo_mini_adas::config::*;
use qor_feo::activity::Activity;
use std::sync::{Arc, Mutex};

use feo_mini_adas::activities::components::{
    EnvironmentRenderer, NeuralNet
};

use qor_feo::prelude::*;

/// This agent's ID
const AGENT_ID: AgentId = AgentId::new(102);

fn main() {
    feo_logger::init(LevelFilter::Debug, true, true);
    feo_tracing::init(feo_tracing::LevelFilter::TRACE);

    info!("Starting agent {AGENT_ID}");

    let neural_net_act: Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(NeuralNet::build(
        3.into(),
        TOPIC_CAMERA_FRONT,
        TOPIC_RADAR_FRONT,
        TOPIC_INFERRED_SCENE,
    )));
    let environ_renderer_act: Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(
        EnvironmentRenderer::build(4.into(), TOPIC_INFERRED_SCENE),
    ));

    let activities = vec![neural_net_act, environ_renderer_act];
    let concurrency = vec![true,false]; // TRUE: if the activities of the AGENT is independent within agent's context

    let agent = Agent::new(2, &activities, concurrency, Engine::default());

    agent.run();
}
