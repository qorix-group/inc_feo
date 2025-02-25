// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use configuration::secondary_agent::Builder;
use feo::configuration::worker_pool;
use feo::prelude::*;
use feo_log::{info, LevelFilter};
use feo_mini_adas::config::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use qor_feo::prelude::{Activity,ActivityId};
use std::{sync::{Arc, Mutex}};

use feo_mini_adas::activities::components::{
    BrakeController, Camera, EmergencyBraking, EnvironmentRenderer, LaneAssist, NeuralNet, Radar,
    SteeringController,
};

use qor_feo::prelude::*;

/// This agent's ID
const AGENT_ID: AgentId = AgentId::new(101);
/// Address of the primary agent
const PRIMARY_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081);

fn main() {
    feo_logger::init(LevelFilter::Debug, true, true);
    feo_tracing::init(feo_tracing::LevelFilter::TRACE);

    info!("Starting agent {AGENT_ID}");


    let emg_brk_Act:Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(EmergencyBraking::build(5.into(), TOPIC_INFERRED_SCENE, TOPIC_CONTROL_BRAKES)));
    let brk_ctr_Act:Arc<Mutex<dyn Activity>> =Arc::new(Mutex::new(BrakeController::build(6.into(), TOPIC_CONTROL_BRAKES)));
    let lane_asst_Act:Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(LaneAssist::build(7.into(), TOPIC_INFERRED_SCENE, TOPIC_CONTROL_STEERING)));
    let str_ctr_Act:Arc<Mutex<dyn Activity>> =Arc::new(Mutex::new(SteeringController::build(8.into(), TOPIC_CONTROL_STEERING)));



    let activities = vec![emg_brk_Act,brk_ctr_Act,lane_asst_Act,str_ctr_Act];
    
    let agent = Agent::new(3,&activities);

    agent.run();

    // // Create worker pool builder activity builder for local worker pool
    // let mut worker_pool_builder = worker_pool::Builder::default();

    // let mut worker_pool_configuration = config::pool_configuration();
    // let assignments = worker_pool_configuration
    //     .remove(&AGENT_ID)
    //     .expect("missing agent id in pool configuration");

    // // Assign activities to workers
    // for (worker_id, activities) in assignments {
    //     for (activity_id, builder) in activities {
    //         worker_pool_builder.activity(worker_id, activity_id, builder);
    //     }
    // }

    // let (worker_pool, _, receiver) = worker_pool_builder.build().expect("Worker pool is empty");

    // // Construct the agent
    // let agent = Builder::default()
    //     .id(AGENT_ID)
    //     .primary(PRIMARY_ADDR)
    //     .worker_pool(worker_pool, receiver)
    //     .build();

    // // Start the agent loop and never return.
    // secondary::run(agent);
}
