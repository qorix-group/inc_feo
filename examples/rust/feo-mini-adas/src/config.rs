// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use crate::activities::messages::{BrakeInstruction, CameraImage, RadarScan, Scene, Steering};
use configuration::topics::Direction;
use feo::com::{init_topic, TopicHandle};
use feo::configuration::topics::TopicSpecification;
use feo::prelude::*;

pub type WorkerAssignment = (WorkerId, Vec<(ActivityId, Box<dyn ActivityBuilder>)>);
pub type AgentAssignment = (AgentId, Vec<WorkerAssignment>);

// For each activity list the activities it needs to wait for
// pub type ActivityDependencies = HashMap<ActivityId, Vec<ActivityId>>;

pub const TOPIC_INFERRED_SCENE: &str = "feo/com/vehicle/inferred/scene";
pub const TOPIC_CONTROL_BRAKES: &str = "feo/com/vehicle/control/brakes";
pub const TOPIC_CONTROL_STEERING: &str = "feo/com/vehicle/control/steering";
pub const TOPIC_CAMERA_FRONT: &str = "feo/com/vehicle/camera/front";
pub const TOPIC_RADAR_FRONT: &str = "feo/com/vehicle/radar/front";

// pub fn activity_dependencies() -> ActivityDependencies {
//     //      Primary              |       Secondary1         |                  Secondary2
//     // ---------------------------------------------------------------------------------------------------
//     //
//     //   Camera(40)   Radar(41)
//     //        \           \
//     //                                 NeuralNet(42)
//     //                                      |                           \                     \
//     //                             EnvironmentRenderer(42)       EmergencyBraking(43)    LaneAssist(44)
//     //                                                                   |                     |
//     //                                                            BrakeController(43)   SteeringController(44)

pub fn initialize_topics() -> Vec<TopicHandle> {
    topic_dependencies()
        .into_iter()
        .map(|spec| {
            let writers = spec
                .peers
                .iter()
                .filter(|(_, dir)| matches!(dir, Direction::Outgoing))
                .count();
            let readers = spec
                .peers
                .iter()
                .filter(|(_, dir)| matches!(dir, Direction::Incoming))
                .count();

            (spec.init_fn)(writers, readers)
        })
        .collect()
}

fn topic_dependencies() -> Vec<TopicSpecification> {
    use Direction::*;
    vec![
        TopicSpecification {
            peers: vec![(0.into(), Outgoing), (2.into(), Incoming)],
            init_fn: Box::new(|w, r| init_topic::<CameraImage>(TOPIC_CAMERA_FRONT, w, r)),
        },
        TopicSpecification {
            peers: vec![(1.into(), Outgoing), (2.into(), Incoming)],
            init_fn: Box::new(|w, r| init_topic::<RadarScan>(TOPIC_RADAR_FRONT, w, r)),
        },
        TopicSpecification {
            peers: vec![
                (2.into(), Outgoing),
                (3.into(), Incoming),
                (4.into(), Incoming),
                (5.into(), Incoming),
            ],
            init_fn: Box::new(|w, r| init_topic::<Scene>(TOPIC_INFERRED_SCENE, w, r)),
        },
        TopicSpecification {
            peers: vec![(4.into(), Outgoing), (6.into(), Incoming)],
            init_fn: Box::new(|w, r| init_topic::<BrakeInstruction>(TOPIC_CONTROL_BRAKES, w, r)),
        },
        TopicSpecification {
            peers: vec![(5.into(), Outgoing), (7.into(), Incoming)],
            init_fn: Box::new(|w, r| init_topic::<Steering>(TOPIC_CONTROL_STEERING, w, r)),
        },
    ]
}
