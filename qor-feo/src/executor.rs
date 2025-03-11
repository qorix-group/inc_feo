// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;

use crate::Agent;

use std::collections::HashMap;
use std::time::Duration;

fn generate_ipc_events(names: &[&str]) -> HashMap<String, HashMap<String, Event<IpcEvent>>> {
    let mut events_map: HashMap<String, HashMap<String, Event<IpcEvent>>> = HashMap::new();

    for &name in names {
        let mut event_submap: HashMap<String, Event<IpcEvent>> = HashMap::new();

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

        events_map.insert(name.to_string(), event_submap);
    }

    events_map
}

fn generate_agent_events(names: &[&str]) -> HashMap<String, Event<IpcEvent>> {
    let mut events_map: HashMap<String, Event<IpcEvent>> = HashMap::new();

    for &name in names {
        events_map.insert(
            format!("{}_agent", name).to_string(),
            IpcEvent::new(&format!("{}_agent", name)),
        );
    }

    events_map
}

pub struct Executor<'a> {
    engine: Engine,
    ipc_events: HashMap<String, HashMap<String, Event<IpcEvent>>>,
    agent_events: HashMap<String, Event<IpcEvent>>,
    names: Vec<&'a str>,
    agents: Vec<&'a str>,
    timer_event: Event<SingleEvent>,
    cycle_time: Duration,
    stop_event: Event<SingleEvent>,
    agent: Agent<'a>,
}

impl<'a> Executor<'a> {
    pub fn new(
        names: &'a [&'a str],
        agents: &'a [&'a str],
        cycle_time: Duration,
        agent: Agent<'a>,
        engine: Engine,
    ) -> Self {
        Self {
            engine: engine,
            ipc_events: generate_ipc_events(names),
            agent_events: generate_agent_events(agents),
            names: names.to_vec(),
            agents: agents.to_vec(),
            timer_event: SingleEvent::new(),
            cycle_time: cycle_time,
            stop_event: SingleEvent::new(),
            agent: agent,
        }
    }

    fn init(&self, names: &[&str]) -> Box<dyn Action> {
        let mut top_sequence = Sequence::new();

        let mut trigger_concurrency = Concurrency::new();
        for &name in names {
            let sub_sequence = Sequence::new().with_step(Trigger::new(
                self.ipc_events
                    .get(name)
                    .unwrap()
                    .get("startup")
                    .unwrap()
                    .notifier()
                    .unwrap(),
            ));

            trigger_concurrency = trigger_concurrency.with_branch(sub_sequence);
        }
        top_sequence = top_sequence.with_step(trigger_concurrency);

        let mut ack_concurrency = Concurrency::new();
        for &name in names {
            let sub_sequence = Sequence::new().with_step(Sync::new(
                self.ipc_events
                    .get(name)
                    .unwrap()
                    .get("startup_ack")
                    .unwrap()
                    .listener()
                    .unwrap(),
            ));
            ack_concurrency = ack_concurrency.with_branch(sub_sequence);
        }

        top_sequence = top_sequence.with_step(ack_concurrency);

        return top_sequence;
    }

    fn step(&self, names: Vec<&str>) -> Box<dyn Action> {
        let mut sequence = Sequence::new();
        for name in names {
            sequence = sequence
            .with_step(Trigger::new(
                self.ipc_events
                    .get(name)
                    .unwrap()
                    .get("step")
                    .unwrap()
                    .notifier()
                    .unwrap(),
            ))
            .with_step(Sync::new(
                self.ipc_events
                    .get(name)
                    .unwrap()
                    .get("step_ack")
                    .unwrap()
                    .listener()
                    .unwrap(),
            ));
        }
        return sequence;
    }

    fn terminate(&self, names: &[&str]) -> Box<dyn Action> {
        let mut top_sequence = Sequence::new();

        let mut trigger_concurrency = Concurrency::new();
        for &name in names {
            let sub_sequence = Sequence::new().with_step(Trigger::new(
                self.ipc_events
                    .get(name)
                    .unwrap()
                    .get("shutdown")
                    .unwrap()
                    .notifier()
                    .unwrap(),
            ));

            trigger_concurrency = trigger_concurrency.with_branch(sub_sequence);
        }
        top_sequence = top_sequence.with_step(trigger_concurrency);

        let mut ack_concurrency = Concurrency::new();
        for &name in names {
            let sub_sequence = Sequence::new().with_step(Sync::new(
                self.ipc_events
                    .get(name)
                    .unwrap()
                    .get("shutdown_ack")
                    .unwrap()
                    .listener()
                    .unwrap(),
            ));

            ack_concurrency = ack_concurrency.with_branch(sub_sequence);
        }
        top_sequence = top_sequence.with_step(ack_concurrency);

        return top_sequence;
    }

    fn sync_to_agents(&self, agents: &[&str]) -> Box<dyn Action> {
        let mut top_sequence = Concurrency::new();

        for &name in agents {
            let sub_sequence = Sync::new(
                self.agent_events
                    .get(&format!("{}_agent", name))
                    .unwrap()
                    .listener()
                    .unwrap(),
            );

            top_sequence = top_sequence.with_branch(sub_sequence);
        }

        top_sequence
    }

    fn timer_run(&self) -> Box<dyn Action> {
        Loop::new().with_body(
            Sequence::new()
                .with_step(Sleep::new(self.cycle_time))
                .with_step(Trigger::new(self.timer_event.notifier().unwrap())),
        )
    }

    pub fn stop_trigger(&self) {
        self.stop_event.notifier().unwrap().notify();
    }

    pub fn run(&self, graph: &Vec<(Vec<&str>, bool)>) {

        let pgminit = Program::new().with_action(
            Sequence::new()
                .with_step(self.sync_to_agents(&self.agents))
                .with_step(self.init(&self.names))
                .with_step(
                    Computation::new()
                        .with_branch(self.timer_run())
                        .with_branch(Sync::new(self.stop_event.listener().unwrap()))
                        .with_branch(
                            Loop::new().with_body(
                                Sequence::new()
                                    .with_step(Sync::new(self.timer_event.listener().unwrap()))
                                    .with_step(self.dependency_graph_to_execution(graph)),
                            ),
                        ),
                )
                .with_step(self.terminate(&self.names)),
        );

        println!("Executor starts syncing with agents and execution of activity chain...");
        self.engine.start().unwrap();
        let handle = pgminit.spawn(&self.engine).unwrap();
        let pgms = self.agent.agent_program();
        let mut handles = Vec::new();
        for pgm in pgms {
            handles.push(pgm.spawn(&self.engine).unwrap());
        }

        // here we wait for some time for the demo
        println!("Waiting for 15-secs...");
        std::thread::sleep(Duration::from_secs(15));

        println!("Reached 15-secs, stopping...");

        self.stop_trigger();

        // Wait for the agent programs to finish
        for hndl in handles {
            let _ = hndl.join().unwrap();
        }

        // Wait for the executor program to finish
        let _ = handle.join().unwrap();

        println!("Done");
    }

    /// Converts a dependency graph into an execution sequence.
    pub fn dependency_graph_to_execution(
        &self,
        execution_structure: &Vec<(Vec<&str>, bool)>
    ) -> Box<dyn Action> {
        let mut sequence = Sequence::new(); // The overall execution sequence
        let mut concurrency_action = Concurrency::new();

        let mut concurrent_block_added = false;

        for task_group in execution_structure {
            if task_group.1 == false {
                // Add the concurrency block into sequence
                if concurrent_block_added {
                    sequence = sequence.with_step(concurrency_action);
                    concurrency_action = Concurrency::new();
                    concurrent_block_added = false;
                }
                // sequence                
                let action = self.step(task_group.0.clone());
                sequence = sequence.with_step(action);
            } else {
                // concurrency block
                let action = self.step(task_group.0.clone());
                concurrency_action = concurrency_action.with_branch(action);
                concurrent_block_added = true;
            }
        }
        if concurrent_block_added {
            sequence = sequence.with_step(concurrency_action);
        }
        sequence as Box<dyn Action>
    }
}
