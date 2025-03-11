// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use feo::prelude::*;
use qor_feo::prelude::*;

use qor_feo::prelude::Activity;

use feo_log::{info, LevelFilter};
use feo_mini_adas::config::*;
use feo_time::Duration;
use feo_mini_adas::activities::components::{
    Camera, Radar
};
use std::sync::{Arc, Mutex};

const AGENT_ID: AgentId = AgentId::new(100);

fn main() {
    feo_logger::init(LevelFilter::Debug, true, true);
    feo_tracing::init(feo_tracing::LevelFilter::TRACE);

    // let params = Params::from_args();
    // Initialize topics. Do not drop.
    let _topic_guards = initialize_topics();

    info!("Starting primary agent {AGENT_ID}. Waiting for connections",);

    let cam_activity: &str = &1.to_string();
    let radar_activity: &str = &2.to_string();
    let neural_net_act: &str = &3.to_string();
    let environ_renderer_act: &str = &4.to_string();
    let emg_brk_act: &str = &5.to_string();
    let brk_ctr_act: &str = &6.to_string();
    let lane_asst_act: &str = &7.to_string();
    let str_ctr_act: &str = &8.to_string();

    let agent_one: &str = &1.to_string();
    let agent_two: &str = &2.to_string();
    let agent_three: &str = &3.to_string();

    let names: Vec<&str> = vec![
        cam_activity,
        radar_activity,
        neural_net_act,
        environ_renderer_act,
        emg_brk_act,
        brk_ctr_act,
        lane_asst_act,
        str_ctr_act,
    ];
    let agents: Vec<&str> = vec![agent_one, agent_two, agent_three];

    // VEC of activitie(s) which has to be executed in sequence, TRUE: if the activitie(s) can be executed concurrently.
    let execution_structure = vec![
        (vec![cam_activity], true),
        (vec![radar_activity], true),
        (vec![neural_net_act], false),
        (vec![environ_renderer_act], true),
        (vec![emg_brk_act, brk_ctr_act], true),
        (vec![lane_asst_act, str_ctr_act], true),
    ];

    //Agent setup

    let cam_act: Arc<Mutex<dyn Activity>> =
        Arc::new(Mutex::new(Camera::build(1.into(), TOPIC_CAMERA_FRONT)));
    let radar_act: Arc<Mutex<dyn Activity>> =
        Arc::new(Mutex::new(Radar::build(2.into(), TOPIC_RADAR_FRONT)));

    let activities = vec![cam_act, radar_act];
    let concurrency = vec![true,true]; // TRUE: if the activities of the AGENT is independent within agent's context

    let agent = Agent::new(1, &activities, concurrency, Engine::default());

    let exec = Executor::new(
        &names,
        &agents,
        Duration::from_millis(500),
        agent,
        Engine::default(),
    );

    exec.run(&execution_structure);

}
