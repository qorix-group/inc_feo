// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use feo::prelude::*;
use feo_log::{info, LevelFilter};
use feo_mini_adas::config::*;
use qor_feo::prelude::Activity;
use std::sync::{Arc, Mutex};

use feo_mini_adas::activities::components::{
    BrakeController, EmergencyBraking, LaneAssist, SteeringController
};

use qor_feo::prelude::*;

/// This agent's ID
const AGENT_ID: AgentId = AgentId::new(101);

fn main() {
    feo_logger::init(LevelFilter::Debug, true, true);
    feo_tracing::init(feo_tracing::LevelFilter::TRACE);

    info!("Starting agent {AGENT_ID}");

    let emg_brk_act: Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(EmergencyBraking::build(
        5.into(),
        TOPIC_INFERRED_SCENE,
        TOPIC_CONTROL_BRAKES,
    )));
    let brk_ctr_act: Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(BrakeController::build(
        6.into(),
        TOPIC_CONTROL_BRAKES,
    )));
    let lane_asst_act: Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(LaneAssist::build(
        7.into(),
        TOPIC_INFERRED_SCENE,
        TOPIC_CONTROL_STEERING,
    )));
    let str_ctr_act: Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(SteeringController::build(
        8.into(),
        TOPIC_CONTROL_STEERING,
    )));

    let activities = vec![emg_brk_act, brk_ctr_act, lane_asst_act, str_ctr_act];

    let agent = Agent::new(3, &activities, Engine::default());

    agent.run();

}
