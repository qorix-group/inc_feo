// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;

use crate::Agent;

use std::{
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};
use std::collections::HashMap;

use std::collections::{HashSet, VecDeque};



fn generate_ipc_events(names: &[&str]) -> HashMap<String, HashMap<String, Event<IpcEvent>>> {
    let mut events_map: HashMap<String, HashMap<String, Event<IpcEvent>>> = HashMap::new();

    for &name in names {
        let mut event_submap: HashMap<String, Event<IpcEvent>>= HashMap::new();
        println!("{}" ,name);

        event_submap.insert("startup".to_string(), IpcEvent::new(&format!("{}_startup", name)));
        event_submap.insert("startup_ack".to_string(), IpcEvent::new(&format!("{}_startup_ack", name)));
        event_submap.insert("step".to_string(), IpcEvent::new(&format!("{}_step", name)));
        event_submap.insert("step_ack".to_string(), IpcEvent::new(&format!("{}_step_ack", name)));
        event_submap.insert("shutdown".to_string(), IpcEvent::new(&format!("{}_shutdown", name)));
        event_submap.insert("shutdown_ack".to_string(), IpcEvent::new(&format!("{}_shutdown_ack", name)));

        events_map.insert(name.to_string(), event_submap);
    }

    events_map
}

fn generate_agent_events(names: &[&str]) -> HashMap<String, Event<IpcEvent>>{
    let mut events_map: HashMap<String,Event<IpcEvent>> = HashMap::new();

    for &name in names {
        events_map.insert(format!("{}_agent", name).to_string(), IpcEvent::new(&format!("{}_agent", name)));
    }

    events_map
}



pub struct Executor<'a> {
    engine: Engine,
    ipc_events:HashMap<String, HashMap<String, Event<IpcEvent>>>,
    agent_events:HashMap<String, Event<IpcEvent>>,
    names: Vec<&'a str>,
    agents: Vec<&'a str>,
    timer_event:Event<SingleEvent>,
    cycle_time:Duration,
    stop_event:Event<SingleEvent>,
    agent:Agent<'a>,
}

impl<'a> Executor<'a> {
    //should take the task chain as input later
    pub fn new(names: &'a[&'a str],agents:&'a[&'a str],cycle_time:Duration,agent:Agent<'a>) -> Self {
        Self {
            engine: Engine::default(),
            ipc_events:generate_ipc_events(names),
            agent_events:generate_agent_events(agents),
            names:names.to_vec(),
            agents:agents.to_vec(),
            timer_event:SingleEvent::new(),
            cycle_time:cycle_time,
            stop_event:SingleEvent::new(),
            agent:agent,
        }
    }


    fn init(&self,names: &[&str])-> Box<dyn Action>{

        let mut top_sequence = Sequence::new();
        
         for &name in names {
        
            let sub_sequence =         Sequence::new()
            .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("startup").unwrap().notifier().unwrap()))
            .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("startup_ack").unwrap().listener().unwrap()));
        
            top_sequence= top_sequence.with_step(sub_sequence);
        
         }
    
         top_sequence
    }

    fn step(&self,name:&str
    ) -> Box<dyn Action> {
        println!("name- {}",name);
            Sequence::new()
                .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("step").unwrap().notifier().unwrap()))
                .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("step_ack").unwrap().listener().unwrap()))
    }



    fn terminate(&self,names: &[&str]
    ) -> Box<dyn Action> {
        let mut top_sequence = Sequence::new();
        
         for &name in names {
        
            let sub_sequence =         Sequence::new()
            .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("shutdown").unwrap().notifier().unwrap()))
            .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("shutdown_ack").unwrap().listener().unwrap()));
        
            top_sequence= top_sequence.with_step(sub_sequence);
        
         }
    
         top_sequence
    }

    fn sync_to_agents(&self,agents: &[&str])-> Box<dyn Action> {

        let mut top_sequence = Sequence::new();
        
         for &name in agents {
        
            let sub_sequence =Sync::new(self.agent_events.get(&format!("{}_agent", name)).unwrap().listener().unwrap());
        
            top_sequence= top_sequence.with_step(sub_sequence);
        
         }
    
         top_sequence
    }

    fn timer_run(&self)->Box<dyn Action> {
            Loop::new().with_body(
                Sequence::new()
                    .with_step(Sleep::new(self.cycle_time))
                    .with_step(Trigger::new(self.timer_event.notifier().unwrap())),
            )

    }

    pub fn stop_trigger(&self){

        self.stop_event.notifier().unwrap().notify();
    }

    pub fn run(&self,graph: &HashMap<&str, Vec<&str>>) {
        self.engine.start().unwrap();

        println!("reach exec run");

        let pgminit = Program::new().with_action(
            Sequence::new()
            .with_step(
                self.sync_to_agents(&self.agents),
            )
            .with_step(
                self.init(&self.names),
            )
            .with_step(
                Computation::new()
                .with_branch(self.timer_run())
                .with_branch(Sync::new(self.stop_event.listener().unwrap()))
                .with_branch(
                    Loop::new().with_body(
                    Sequence::new()
                    .with_step(Sync::new(self.timer_event.listener().unwrap()))
                    .with_step(
                        self.dependency_graph_to_sequence(graph),
                    ),
                )
                ),
            )
            .with_step(
                self.terminate(&self.names),
            ),
        );

        self.timer_run();
        println!("before run");
        let handle = pgminit.spawn(&self.engine).unwrap();
        let handle_agent = self.agent.agent_program().spawn(&self.engine).unwrap();

        // here we wait for some time for the demo
        std::thread::sleep(Duration::from_secs(15));

        println!("reached 5sec");

        self.stop_trigger();


        let _ = handle_agent.join().unwrap();
        // Wait for the program to finish
        let _ = handle.join().unwrap();


        println!("Done");
    }

/// Converts a dependency graph into an execution sequence.
fn dependency_graph_to_sequence(&self,graph: &HashMap<&str, Vec<&str>>) -> Box<dyn Action> {
    let mut in_degree = HashMap::new();
    let mut adj_list = HashMap::new();

    // Initialize in-degree and adjacency list
    for (&task, deps) in graph.iter() {
        in_degree.entry(task).or_insert(0);
        for &dep in deps {
            *in_degree.entry(dep).or_insert(0) += 1;
            adj_list.entry(dep).or_insert(Vec::new());
        }
        adj_list.insert(task, deps.clone());
    }

    // Queue for tasks with no dependencies (ready to run)
    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &count)| count == 0)
        .map(|(&task, _)| task)
        .collect();

    let mut sequence = Sequence::new();

    // Process tasks level by level
    while !queue.is_empty() {
        let mut current_level = Vec::new();
        let mut next_queue = VecDeque::new();

        for &task in queue.iter() {
            current_level.push(task);

            if let Some(dependents) = adj_list.get(task) {
                for &dep in dependents {
                    if let Some(count) = in_degree.get_mut(dep) {
                        *count -= 1;
                        if *count == 0 {
                            next_queue.push_back(dep);
                        }
                    }
                }
            }
        }

        // Add to sequence: single step or concurrent
        if current_level.len() == 1 {
            sequence = sequence.with_step(self.step(current_level[0]));
        } else {
            let mut concurrency = Concurrency::new();
            for task in current_level {
                concurrency = concurrency.with_branch(self.step(task));
            }
            sequence = sequence.with_step(concurrency);
        }

        queue = next_queue;
    }

    sequence
}



}
