// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use configuration::primary_agent::Builder;
use feo::configuration::worker_pool;
use feo::prelude::*;
use qor_feo::prelude::*;

use qor_feo::prelude::{Activity,ActivityId};

use feo::signalling::{channel, Signal};
use feo_log::{info, LevelFilter};
use feo_mini_adas::config::*;
use feo_time::Duration;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use feo_mini_adas::activities::components::{
    BrakeController, Camera, EmergencyBraking, EnvironmentRenderer, LaneAssist, NeuralNet, Radar,
    SteeringController,
};
use std::{sync::{Arc, Mutex}};


const AGENT_ID: AgentId = AgentId::new(100);
const BIND_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8081);
const DEFAULT_FEO_CYCLE_TIME: Duration = Duration::from_secs(5);

fn main() {
    feo_logger::init(LevelFilter::Debug, true, true);
    feo_tracing::init(feo_tracing::LevelFilter::TRACE);

    // let params = Params::from_args();
    // Initialize topics. Do not drop.
    let _topic_guards = initialize_topics();

    info!("Starting primary agent {AGENT_ID}. Waiting for connections",);


    let cam_activity:&str = &1.to_string();
    let radar_activity:&str = &2.to_string();
    let neural_net_activity:&str = &3.to_string();
    let emg_brk_Act:&str = &4.to_string();
    let brk_ctr_Act:&str = &5.to_string();

    let agent_one:&str = &1.to_string();
    let agent_two:&str = &2.to_string();


    let names: Vec<&str> = vec![cam_activity,radar_activity,neural_net_activity,emg_brk_Act,brk_ctr_Act];
    let agents: Vec<&str> = vec![agent_one,agent_two];

    let dependency_graph: HashMap<&str, Vec<&str>> = HashMap::from([
        (cam_activity, vec![]),
         (cam_activity, vec![radar_activity]),     // B depends on A
         (radar_activity, vec![neural_net_activity]),   
         (neural_net_activity, vec![emg_brk_Act]), 
         (emg_brk_Act, vec![brk_ctr_Act]),   // 2a,b depends on b
    ]);

    //Agent setup

    let cam_Act:Arc<Mutex<dyn Activity>> = Arc::new(Mutex::new(Camera::build(1.into(), TOPIC_CAMERA_FRONT)));
    let radar_Act:Arc<Mutex<dyn Activity>> =Arc::new(Mutex::new(Radar::build(2.into(), TOPIC_RADAR_FRONT)));
    let neural_net_Act:Arc<Mutex<dyn Activity>> =Arc::new(Mutex::new(NeuralNet::build(3.into(),TOPIC_CAMERA_FRONT,TOPIC_RADAR_FRONT,TOPIC_INFERRED_SCENE)));
    // EmergencyBraking::build(id, TOPIC_INFERRED_SCENE, TOPIC_CONTROL_BRAKES);
    // BrakeController::build(id, TOPIC_CONTROL_BRAKES);


    let activities = vec![cam_Act,radar_Act,neural_net_Act];
    
    let agent = Agent::new(1,&activities);

    //



    let exec = Executor::new(&names,&agents,Duration::from_millis(500),agent);

    exec.run(&dependency_graph);




    // // Create local worker pool
    // let (worker_pool, agent_map, ready_channel) = {
    //     let pool_configuration = config::pool_configuration();
    //     let mut worker_pool_builder = worker_pool::Builder::default();
    //     let mut agent_map: HashMap<AgentId, HashMap<WorkerId, Vec<ActivityId>>> = HashMap::new();

    //     // Recreate the HashMap without the builder on the lowest level.
    //     for (agent_id, assignments) in pool_configuration.into_iter() {
    //         for (worker_id, activities) in assignments.into_iter() {
    //             for (activity_id, builder) in activities {
    //                 if agent_id == AGENT_ID {
    //                     worker_pool_builder.activity(worker_id, activity_id, builder);
    //                 }

    //                 // Reinsert with same structure but without the builder on the lowest level.
    //                 agent_map
    //                     .entry(agent_id)
    //                     .or_default()
    //                     .entry(worker_id)
    //                     .and_modify(|act_ids| act_ids.push(activity_id))
    //                     .or_insert_with(|| vec![activity_id]);
    //             }
    //         }
    //     }

    //     let (worker_pool, ready_channel) = match worker_pool_builder.build() {
    //         Some((pool, sender, receiver)) => (Some(pool), (sender, receiver)),
    //         None => {
    //             let ready_channel = channel::<Signal>();
    //             (None, ready_channel)
    //         }
    //     };

    //     (worker_pool, agent_map, ready_channel)
    // };

    // let activity_dependencies = config::activity_dependencies();

    // // Construct the agent
    // let agent = Builder::default()
    //     .id(AGENT_ID)
    //     .cycle_time(params.feo_cycle_time)
    //     .bind(BIND_ADDR)
    //     .agent_map(agent_map)
    //     .worker_pool(worker_pool)
    //     .activity_dependencies(activity_dependencies)
    //     .intra_proc_ready_channel(ready_channel.0, ready_channel.1)
    //     .build();

    // // Start the agent loop and never return.
    // primary::run(agent);
}

// Parameters of the primary
// struct Params {
//     /// Cycle time in milli seconds
//     feo_cycle_time: Duration,
// }

// impl Params {
//     fn from_args() -> Self {
//         let args: Vec<String> = std::env::args().collect();

//         let feo_cycle_time = args
//             .get(1)
//             .and_then(|x| x.parse::<u64>().ok())
//             .map(Duration::from_millis)
//             .unwrap_or(DEFAULT_FEO_CYCLE_TIME);

//         Self { feo_cycle_time }
//     }
// }
