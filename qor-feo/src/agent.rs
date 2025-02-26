// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use crate::activity::Activity;

use qor_rto::prelude::*;
use std::{
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};
use std::collections::HashMap;
use std::collections::HashSet;

fn generate_ipc_events(activities: &Vec<Arc<Mutex<dyn Activity>>>) -> HashMap<String, HashMap<String, Event<IpcEvent>>> {
    let mut events_map: HashMap<String, HashMap<String, Event<IpcEvent>>> = HashMap::new();

    for activity in activities.iter() {
        let mut event_submap: HashMap<String, Event<IpcEvent>>= HashMap::new();
        let name:&str= &activity.lock().unwrap().getname();
        println!("{} - 1" ,name);
        event_submap.insert("startup".to_string(), IpcEvent::new(&format!("{}_startup", name)));
        event_submap.insert("startup_ack".to_string(), IpcEvent::new(&format!("{}_startup_ack", name)));
        event_submap.insert("step".to_string(), IpcEvent::new(&format!("{}_step", name)));
        event_submap.insert("step_ack".to_string(), IpcEvent::new(&format!("{}_step_ack", name)));
        event_submap.insert("shutdown".to_string(), IpcEvent::new(&format!("{}_shutdown", name)));
        event_submap.insert("shutdown_ack".to_string(), IpcEvent::new(&format!("{}_shutdown_ack", name)));

        events_map.insert(activity.lock().unwrap().getname(), event_submap);
    }

    events_map
}



pub struct Agent<'a>{
    id:usize,
    engine: Engine,
    ipc_events:HashMap<String, HashMap<String, Event<IpcEvent>>>,
    agent_event:HashMap<String, Event<IpcEvent>>,
    activities: &'a Vec<Arc<Mutex<dyn Activity>>>
}

impl<'a> Agent<'a> {
    //should take the task chain as input later
    pub fn new(id:usize,this: &'a Vec<Arc<Mutex<dyn Activity>>>,engine:Engine) -> Self {
        let mut events_map: HashMap<String,Event<IpcEvent>> = HashMap::new();
        events_map.insert(format!("{}_agent", id.to_string()).to_string(), IpcEvent::new(&format!("{}_agent", id.to_string())));
        Self {
            id:id,
            engine: engine,
            ipc_events:generate_ipc_events(this),
            agent_event:events_map,
            activities: this
        }
    }

    fn startup(&self)-> Box<dyn Action>{

        let mut top_sequence = Sequence::new();
        
         for activity in self.activities.iter() {
            let name= &activity.lock().unwrap().getname();
            let sub_sequence =         Sequence::new()
            .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("startup").unwrap().listener().unwrap()))
            .with_step(Await::new_method_mut_u(activity, Activity::startup))
            .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("startup_ack").unwrap().notifier().unwrap()));

            top_sequence= top_sequence.with_step(sub_sequence);
     
         }

         top_sequence
    }

    fn step(&self)-> Box<dyn Action>{

        let mut top_sequence = Concurrency::new();
        
         for activity in self.activities {
            let name= &activity.lock().unwrap().getname();
            let sub_sequence =         Sequence::new()
            .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("step").unwrap().listener().unwrap()))
            .with_step(Await::new_method_mut_u(activity, Activity::step))
            .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("step_ack").unwrap().notifier().unwrap()));


            top_sequence= top_sequence.with_branch(sub_sequence);
        
         }
    
         top_sequence
    }

    fn shutdown(&self)-> Box<dyn Action>{

        let mut top_sequence = Sequence::new();
        
         for activity in self.activities.iter() {
            let name= &activity.lock().unwrap().getname();
            let sub_sequence =         Sequence::new()
            .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("shutdown").unwrap().listener().unwrap()))
            .with_step(Await::new_method_mut_u(&activity.clone(), Activity::shutdown))
            .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("shutdown_ack").unwrap().notifier().unwrap()));


            top_sequence= top_sequence.with_step(sub_sequence);
        
         }
    
         top_sequence
    }
    fn connect_to_executor(&self)-> Box<dyn Action>{
        println!("agent - {}_agent",self.id.to_string());
        Sequence::new()
        .with_step(Trigger::new(self.agent_event.get(&format!("{}_agent", self.id.to_string())).unwrap().notifier().unwrap()))
    }

    pub fn agent_program(&self)-> Program {
        Program::new().with_action(
            Sequence::new()
            .with_step(
               self.connect_to_executor(),
   )
                //step
                .with_step(
                       self.startup(),
           )
            .with_step(
               Computation::new()
                .with_branch(Loop::new().with_body(self.step(),))
                .with_branch(self.shutdown()),
            ),
   )

    }

    pub fn run(&self){
        self.engine.start().unwrap();
        println!("reach");

        let pgminit = self.agent_program();

        
        let handle = pgminit.spawn(&self.engine).unwrap();

                // Wait for the program to finish
        let _ = handle.join().unwrap();

    }


}
