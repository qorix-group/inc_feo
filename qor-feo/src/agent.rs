// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use crate::activity::Activity;

use qor_rto::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn generate_ipc_events(
    activities: &Vec<Arc<Mutex<dyn Activity>>>,
) -> HashMap<String, HashMap<String, Event<IpcEvent>>> {
    let mut events_map: HashMap<String, HashMap<String, Event<IpcEvent>>> = HashMap::new();

    for activity in activities.iter() {
        let mut event_submap: HashMap<String, Event<IpcEvent>> = HashMap::new();
        let name: &str = &activity.lock().unwrap().getname();
        event_submap.insert(
            "startup".to_string(),
            IpcEvent::new(&format!("{}_startup", name)),
        );
        event_submap.insert(
            "startup_ack".to_string(),
            IpcEvent::new(&format!("{}_startup_ack", name)),
        );
        event_submap.insert("step".to_string(), IpcEvent::new(&format!("{}_step", name)));
        event_submap.insert(
            "step_ack".to_string(),
            IpcEvent::new(&format!("{}_step_ack", name)),
        );
        event_submap.insert(
            "shutdown".to_string(),
            IpcEvent::new(&format!("{}_shutdown", name)),
        );
        event_submap.insert(
            "shutdown_ack".to_string(),
            IpcEvent::new(&format!("{}_shutdown_ack", name)),
        );

        events_map.insert(activity.lock().unwrap().getname(), event_submap);
    }

    events_map
}

pub struct Agent<'a> {
    id: usize,
    engine: Engine,
    ipc_events: HashMap<String, HashMap<String, Event<IpcEvent>>>,
    agent_event: HashMap<String, Event<IpcEvent>>,
    activities: &'a Vec<Arc<Mutex<dyn Activity>>>,
    concurrency: Vec<bool>,
}

impl<'a> Agent<'a> {
    //should take the task chain as input later
    pub fn new(id: usize, this: &'a Vec<Arc<Mutex<dyn Activity>>>, concurrency: Vec<bool>, engine: Engine) -> Self {
        let mut events_map: HashMap<String, Event<IpcEvent>> = HashMap::new();
        events_map.insert(
            format!("{}_agent", id.to_string()).to_string(),
            IpcEvent::new(&format!("{}_agent", id.to_string())),
        );
        Self {
            id: id,
            engine: engine,
            ipc_events: generate_ipc_events(this),
            agent_event: events_map,
            activities: this,
            concurrency: concurrency,
        }
    }

    fn startup(&self, activity_index: &Vec<usize>) -> Box<dyn Action> {
        let mut top_sequence = Concurrency::new();

        for index in activity_index {
            let activity = self.activities[index.clone()].clone();
            let name = &activity.lock().unwrap().getname();
            let sub_sequence = Sequence::new()
                .with_step(Sync::new(
                    self.ipc_events
                        .get(name)
                        .unwrap()
                        .get("startup")
                        .unwrap()
                        .listener()
                        .unwrap(),
                ))
                .with_step(Invoke::new(move|_| {
                    let _ = activity.lock().unwrap().startup();
                    (Duration::ZERO, UpdateResult::Complete)
                }))
                .with_step(Trigger::new(
                    self.ipc_events
                        .get(name)
                        .unwrap()
                        .get("startup_ack")
                        .unwrap()
                        .notifier()
                        .unwrap(),
                ));

            top_sequence = top_sequence.with_branch(sub_sequence);
        }

        top_sequence
    }

    fn step(&self, activity_index: &Vec<usize>) -> Box<dyn Action> {
        let mut top_sequence = Concurrency::new();

        for index in activity_index {
            let activity = self.activities[index.clone()].clone();
            let name = &activity.lock().unwrap().getname();
            let sub_sequence = Sequence::new()
                .with_step(Sync::new(
                    self.ipc_events
                        .get(name)
                        .unwrap()
                        .get("step")
                        .unwrap()
                        .listener()
                        .unwrap(),
                ))
                .with_step(Invoke::new(move|_| {
                    let _ = activity.lock().unwrap().step();
                    (Duration::ZERO, UpdateResult::Complete)
                }))
                .with_step(Trigger::new(
                    self.ipc_events
                        .get(name)
                        .unwrap()
                        .get("step_ack")
                        .unwrap()
                        .notifier()
                        .unwrap(),
                ));

            top_sequence = top_sequence.with_branch(sub_sequence);
        }

        top_sequence
    }

    fn shutdown(&self, activity_index: &Vec<usize>) -> Box<dyn Action> {
        let mut top_sequence = Concurrency::new();

        for index in activity_index {
            let activity = self.activities[index.clone()].clone();
            let name = &activity.lock().unwrap().getname();
            let sub_sequence = Sequence::new()
                .with_step(Sync::new(
                    self.ipc_events
                        .get(name)
                        .unwrap()
                        .get("shutdown")
                        .unwrap()
                        .listener()
                        .unwrap(),
                ))
                .with_step(Invoke::new(move|_| {
                    let _ = activity.lock().unwrap().shutdown();
                    (Duration::ZERO, UpdateResult::Complete)
                }))
                .with_step(Trigger::new(
                    self.ipc_events
                        .get(name)
                        .unwrap()
                        .get("shutdown_ack")
                        .unwrap()
                        .notifier()
                        .unwrap(),
                ));

            top_sequence = top_sequence.with_branch(sub_sequence);
        }

        top_sequence
    }
    fn connect_to_executor(&self) -> Box<dyn Action> {
        println!("Agent : {}_agent", self.id.to_string());
        Sequence::new().with_step(Trigger::new(
            self.agent_event
                .get(&format!("{}_agent", self.id.to_string()))
                .unwrap()
                .notifier()
                .unwrap(),
        ))
    }

    pub fn agent_program(&self) -> Vec<Program> {
        let mut pgms = Vec::new();

        let mut index = 0;

        while index < self.activities.len() {
            let mut sequence: Box<Sequence> = Sequence::new();

            // Add syncing of agents for the first program only
            if index == 0 {
                sequence = sequence.with_step(self.connect_to_executor());
            }

            // Create a program for concurrent activities
            while index < self.activities.len() {
                // First activity is always concurrent
                let mut activities_index = vec![index];
                index += 1;
                while index < self.activities.len() {
                    if self.concurrency[index] == false {
                        activities_index.push(index);
                        index += 1;
                    }
                    else {
                        break;
                    }
                }
                // Create the sequence of the program
                sequence = sequence
                    .with_step(self.startup(&activities_index))
                    .with_step(
                        Computation::new()
                            .with_branch(Loop::new().with_body(self.step(&activities_index)))
                            .with_branch(self.shutdown(&activities_index)),
                    );
                break;
            }
            let pgm = Program::new().with_action(sequence);
            pgms.push(pgm);
        }
        return pgms;
    }

    pub fn run(&self) {
        println!("Agent started and waiting for triggers from executor...");

        let pgms = self.agent_program();

        self.engine.start().unwrap();
        let mut handles = Vec::new();
        for pgm in pgms {
            handles.push(pgm.spawn(&self.engine).unwrap());
        }

        // Wait for the program to finish
        for handle in handles {
            let _ = handle.join().unwrap();
        }
        println!("Done");
    }
}
