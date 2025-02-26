// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use configuration::secondary_agent::Builder;
use feo::configuration::worker_pool;
use feo::prelude::*;
use feo_log::{info, LevelFilter};
use feo_mini_adas::config::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use qor_feo::activity::{Activity,ActivityId};
use std::{sync::{Arc, Mutex}};

use feo_mini_adas::activities::components::{
    BrakeController, Camera, EmergencyBraking, EnvironmentRenderer, LaneAssist, NeuralNet, Radar,
    SteeringController,
};

use qor_feo::prelude::*;

/// This agent's ID
const AGENT_ID: AgentId = AgentId::new(102);
/// Address of the primary agent
const PRIMARY_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081);

fn main() {
    feo_logger::init(LevelFilter::Debug, true, true);
    feo_tracing::init(feo_tracing::LevelFilter::TRACE);

    info!("Starting agent {AGENT_ID}");


    
    let neural_net_act:Arc<Mutex<dyn Activity>> =Arc::new(Mutex::new(NeuralNet::build(3.into(),TOPIC_CAMERA_FRONT,TOPIC_RADAR_FRONT,TOPIC_INFERRED_SCENE)));
    let environ_renderer_act:Arc<Mutex<dyn Activity>> =Arc::new(Mutex::new(EnvironmentRenderer::build(4.into(), TOPIC_INFERRED_SCENE)));
    // EmergencyBraking::build(id, TOPIC_INFERRED_SCENE, TOPIC_CONTROL_BRAKES);
    // BrakeController::build(id, TOPIC_CONTROL_BRAKES);


    let activities = vec![neural_net_act,environ_renderer_act];
    
    let agent = Agent::new(2,&activities,Engine::default());

    agent.run();

}