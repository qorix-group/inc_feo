// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use feo::{ids::ActivityId, orch_adapter::runtime_adapters::ActivityDetailsBuilder};

use crate::{activities::components::*, config::*};

pub mod components;
pub mod messages;

pub fn primary_agent_activities() -> ActivityDetailsBuilder {
    ActivityDetailsBuilder::new("mini-adas-p-design")
        .add_activity(|| Radar::build_orch(1.into(), TOPIC_RADAR_FRONT))
        .add_activity(|| Camera::build_orch(0.into(), TOPIC_CAMERA_FRONT))
}

pub fn secondary1_agent_activities() -> ActivityDetailsBuilder {
    ActivityDetailsBuilder::new("mini-adas-1-design")
        .add_activity(|| EnvironmentRenderer::build_orch(3.into(), TOPIC_INFERRED_SCENE))
        .add_activity(|| {
            NeuralNet::build_orch(
                2.into(),
                TOPIC_CAMERA_FRONT,
                TOPIC_RADAR_FRONT,
                TOPIC_INFERRED_SCENE,
            )
        })
}

pub fn secondary2_agent_activities() -> ActivityDetailsBuilder {
    ActivityDetailsBuilder::new("mini-adas-2-design")
        .add_activity(|| {
            EmergencyBraking::build_orch(4.into(), TOPIC_INFERRED_SCENE, TOPIC_CONTROL_BRAKES)
        })
        .add_activity(|| BrakeController::build_orch(6.into(), TOPIC_CONTROL_BRAKES))
        // .add_activity(|| {
        //     LaneAssist::build_orch(5.into(), TOPIC_INFERRED_SCENE, TOPIC_CONTROL_STEERING)
        // })
        .add_activity(|| SteeringController::build_orch(7.into(), TOPIC_CONTROL_STEERING))
}

pub fn activities_to_agents_assigment() -> HashMap<ActivityId, &'static str> {
    // TODO: Shall come from some config
    let mut assignment = HashMap::new();

    // Primary agent activities
    assignment.insert(1.into(), PRIMARY_SECONDARY_NAME);
    assignment.insert(0.into(), PRIMARY_SECONDARY_NAME);

    // Secondary agent 1 activities
    assignment.insert(3.into(), SECONDARY1_NAME);
    assignment.insert(2.into(), SECONDARY1_NAME);

    // Secondary agent 2 activities
    assignment.insert(4.into(), SECONDARY2_NAME);
    assignment.insert(6.into(), SECONDARY2_NAME);
    assignment.insert(7.into(), SECONDARY2_NAME);

    assignment
}
