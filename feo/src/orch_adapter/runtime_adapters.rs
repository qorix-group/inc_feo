// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use core::{cell::RefCell, ops::DerefMut, time::Duration};
//
// Well known issues:
// - currently activity must be hidden behind Mutex - subject to be lifted
// - !Send issues due to iceoryx
// - ...
//
use std::{
    borrow::ToOwned,
    boxed::Box,
    collections::HashMap,
    format,
    rc::Rc,
    string::String,
    sync::{Arc, Mutex},
    vec,
};

use async_runtime::{core::types::UniqueWorkerId, runtime::async_runtime::AsyncRuntime};
use orchestration::{
    api::{deployment::Deployment, design::Design, OrchestrationApi},
    common::DesignConfig,
    core::metering::Meter,
};

use crate::ids::ActivityId;

use super::types::ActivityAdapterTrait;
use foundation::prelude::*;
use orchestration::prelude::*;
use std::vec::Vec;

pub type ActivityDependencies = HashMap<ActivityId, Vec<ActivityId>>;

type ActivityMethodToActionCreator =
    Box<dyn FnOnce() -> Result<Box<dyn ActionTrait>, CommonErrors>>;

pub struct ActivityDetails {
    start: Option<ActivityMethodToActionCreator>,
    step: Option<ActivityMethodToActionCreator>,
    stop: Option<ActivityMethodToActionCreator>,
    pub id: ActivityId,
    bounded_worker: Option<UniqueWorkerId>,
}

pub struct ActivityDetailsBuilder {
    pub data: Vec<ActivityDetails>,
    design: Design,
}

impl ActivityDetailsBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            data: vec![],
            design: Design::new(name.into(), DesignConfig::default()),
        }
    }

    pub fn add_activity<T: 'static + Send + ActivityAdapterTrait<T = T>, U: FnMut() -> T>(
        mut self,
        mut builder: U,
    ) -> Self {
        let wrapped = Arc::new(Mutex::new(builder()));

        let id = wrapped.lock().unwrap().get_act_id();
        let res = self.activity_into_invokes(&wrapped, &id);
        self.data.push(ActivityDetails {
            start: Some(res.0),
            step: Some(res.1),
            stop: Some(res.2),
            id: id,
            bounded_worker: None,
        });
        self
    }

    pub fn add_bounded_activity<
        T: 'static + Send + ActivityAdapterTrait<T = T>,
        U: FnMut() -> T,
    >(
        mut self,
        builder: U,
        worker: UniqueWorkerId,
    ) -> Self {
        self = self.add_activity(builder);
        self.data.last_mut().unwrap().bounded_worker = Some(worker);
        self
    }

    pub fn build(self) -> (Design, Vec<ActivityDetails>) {
        (self.design, self.data)
    }

    ///
    /// Returns startup, step, shutdown for activity as invoke actions
    ///
    fn activity_into_invokes<T>(
        &mut self,
        obj: &Arc<Mutex<T>>,
        id: &ActivityId,
    ) -> (
        ActivityMethodToActionCreator,
        ActivityMethodToActionCreator,
        ActivityMethodToActionCreator,
    )
    where
        T: 'static + Send + ActivityAdapterTrait<T = T>,
    {
        let start_tag = self.design.register_invoke_method(
            format!("{}/start", id).as_str().into(),
            obj.clone(),
            T::start,
        );

        let step_tag = self.design.register_invoke_method_async(
            format!("{}/step", id).as_str().into(),
            obj.clone(),
            T::step_runtime,
        );

        let stop_tag = self.design.register_invoke_method(
            format!("{}/stop", id).as_str().into(),
            obj.clone(),
            T::stop,
        );

        let design_config = self.design.config().clone();
        (
            Box::new(move || Ok(Invoke::from_tag(&start_tag?, &design_config))),
            Box::new(move || Ok(Invoke::from_tag(&step_tag?, &design_config))),
            Box::new(move || Ok(Invoke::from_tag(&stop_tag?, &design_config))),
        )
    }
}

pub struct FeoRunner {
    feo_app_name: &'static str,
    agents: Vec<FeoAgent>,
    executor: Option<GlobalOrchestrator>,
}

impl FeoRunner {
    pub fn new(feo_app_name: &str) -> Self {
        Self {
            feo_app_name: feo_app_name.to_owned().leak(),
            agents: vec![],
            executor: None,
        }
    }

    pub fn add_agent(&mut self, activities: ActivityDetailsBuilder, agent_name: &'static str) {
        self.agents.push(FeoAgent::new(
            self.feo_app_name,
            activities,
            agent_name,
            false,
        ));
    }

    pub fn with_executor(
        &mut self,
        agents: Vec<String>,
        graph: ActivityDependencies,
        activity_to_agent: HashMap<ActivityId, &'static str>,
    ) {
        self.executor = Some(GlobalOrchestrator::new(
            self.feo_app_name,
            agents,
            graph,
            activity_to_agent,
        ));
    }

    pub fn run(&mut self, runtime: &mut AsyncRuntime, cycle: Duration) {
        // Every agent added here having executor in same place is local agent
        if self.executor.is_some() {
            self.agents.iter_mut().for_each(|a| a.mark_as_local());
        }

        let mut orchestration = OrchestrationApi::new();

        let agents: Vec<FeoAgent> = self.agents.drain(..).collect();
        let mut inners = vec![];

        for mut agent in agents {
            agent.specify_local_agent_design();
            orchestration = orchestration.add_design(agent.design);

            inners.push(agent.inner);
        }

        if let Some(ref executor) = self.executor {
            let design = executor.get_orchestration_design();
            orchestration = orchestration
                .add_design(design.expect("Design shall be valid when provided to runner"));
        }

        let mut orch = orchestration.design_done();

        // Make deployment
        let mut deployment = orch.get_deployment_mut();

        // Deployment for local agents
        for inner in inners {
            FeoAgent::bind_events(
                inner.borrow_mut().deref_mut(),
                self.feo_app_name,
                &mut deployment,
            );
        }

        if let Some(ref executor) = self.executor {
            // Bind global events for executor if there is secondary agent
            executor
                .bind_events(&mut deployment, "secondary_agent")
                .expect("Binding events shall never fail with correct configuration!");
        }

        let mut program_manager = orch.into_program_manager().unwrap();
        let mut programs = program_manager.get_programs();

        runtime
            .block_on(async move {
                let mut handles = vec![];

                while let Some(program) = programs.pop() {
                    let handle = async_runtime::spawn(async move {
                        let mut program = program;
                        if program.name() == "global_orchestrator_program" {
                            // bind events for global orchestrator
                            let _ = program.run_cycle_metered::<Meter>(cycle).await;
                        } else {
                            let _ = program.run().await;
                        }

                        info!("Finished program");
                    });

                    handles.push(handle);
                }

                for handle in handles {
                    handle.await.unwrap();
                }

                Ok(1)
            })
            .unwrap();
    }
}

///
/// Responsible to react on request coming from primary process
///
pub struct FeoAgent {
    design: Design,
    inner: Rc<RefCell<InnerFeoLocal>>,
}

impl FeoAgent {
    pub fn new(
        app_name: &'static str,
        builder: ActivityDetailsBuilder,
        agent_name: &'static str,
        is_local: bool,
    ) -> Self {
        let (design, activities) = builder.build();

        Self {
            design,
            inner: Rc::new(RefCell::new(InnerFeoLocal {
                activities,
                agent_name,
                app_name,
                is_local,
            })),
        }
    }

    fn get_agent_name(&self) -> &'static str {
        self.inner.borrow().agent_name
    }

    fn mark_as_local(&mut self) {
        self.inner.borrow_mut().is_local = true;
    }

    fn specify_local_agent_design(&mut self) {
        // register events
        let instance = self.inner.borrow();

        register_events(
            instance.app_name,
            instance.agent_name,
            &instance.activities,
            &mut self.design,
        )
        .expect("Events registration shall never fail");

        let inner_clone = self.inner.clone();

        self.design.add_program(
            format!("{}Program", self.inner.borrow().agent_name).leak(),
            move |design_instance, builder| {
                let mut instance = inner_clone.borrow_mut();
                builder
                    .with_run_action(Self::create_body(
                        instance.app_name,
                        &mut instance.activities,
                        &design_instance,
                    ))
                    .with_start_action(Self::create_startup(
                        instance.app_name,
                        &mut instance,
                        &design_instance,
                    ))
                    .with_stop_action(
                        Self::create_shutdown(instance.app_name, &mut instance, &design_instance),
                        Duration::from_secs(5),
                    );

                Ok(())
            },
        );
    }

    fn create_body(
        app_name: &str,
        activities: &mut Vec<ActivityDetails>,
        design: &Design,
    ) -> Box<dyn ActionTrait> {
        let mut concurrent = ConcurrencyBuilder::new();

        for e in activities {
            let invokes = e.step.take().unwrap()().unwrap();
            concurrent.with_branch(
                SequenceBuilder::new()
                    .with_step(SyncBuilder::from_design(
                        format!("{}/{}/step", app_name, e.id).as_str(),
                        design,
                    ))
                    .with_step(invokes)
                    .with_step(TriggerBuilder::from_design(
                        format!("{}/{}/step_completed", app_name, e.id).as_str(),
                        design,
                    ))
                    .build(),
            );
        }

        concurrent.build(design)
    }

    fn bind_events(inner: &mut InnerFeoLocal, app_name: &str, deployment: &mut Deployment) {
        bind_helper(
            format!("{}/{}/alive", app_name, inner.agent_name),
            deployment,
            inner.is_local,
        );

        bind_helper(
            format!("{}/{}/startup", app_name, inner.agent_name),
            deployment,
            inner.is_local,
        );
        bind_helper(
            format!("{}/{}/shutdown", app_name, inner.agent_name),
            deployment,
            inner.is_local,
        );

        bind_helper(
            format!("{}/{}/startup_completed", app_name, inner.agent_name,),
            deployment,
            inner.is_local,
        );

        bind_helper(
            format!("{}/{}/shutdown_completed", app_name, inner.agent_name,),
            deployment,
            inner.is_local,
        );

        // register step events for activities
        for e in &inner.activities {
            bind_helper(
                format!("{}/{}/step", app_name, e.id),
                deployment,
                inner.is_local,
            );

            bind_helper(
                format!("{}/{}/step_completed", app_name, e.id),
                deployment,
                inner.is_local,
            );

            if let Some(worker) = e.bounded_worker {
                let res = deployment
                    .bind_invoke_to_worker(format!("{}/step", e.id).as_str().into(), worker);

                assert!(
                    res.is_ok() || (res.unwrap_err() == CommonErrors::AlreadyDone),
                    "Failed to bind to worker {:?}",
                    res
                );
            }
        }
    }

    fn create_startup(
        app_name: &str,
        instance: &mut InnerFeoLocal,
        design: &Design,
    ) -> Box<dyn ActionTrait> {
        let mut seq = SequenceBuilder::new();
        seq.with_step(TriggerBuilder::from_design(
            format!("{}/{}/alive", app_name, instance.agent_name).as_str(),
            design,
        ))
        .with_step(SyncBuilder::from_design(
            format!("{}/{}/startup", app_name, instance.agent_name).as_str(),
            design,
        ));

        let mut concurrent = ConcurrencyBuilder::new();

        // startups from al activities
        for e in &mut instance.activities {
            let invokes = e.start.take().unwrap()().unwrap();
            concurrent.with_branch(invokes);
        }

        seq.with_step(concurrent.build(design))
            .with_step(TriggerBuilder::from_design(
                format!("{}/{}/startup_completed", app_name, instance.agent_name).as_str(),
                design,
            ));

        seq.build()
    }

    // fn create_shutdown_notification(&mut self) -> Box<dyn ActionTrait> {
    //     let seq =
    //         Sequence::new().with_step(Sync::new(format!("{}/shutdown", self.app_name).as_str()));

    //     seq
    // }

    fn create_shutdown(
        app_name: &str,
        instance: &mut InnerFeoLocal,
        design: &Design,
    ) -> Box<dyn ActionTrait> {
        let mut seq = SequenceBuilder::new();

        let mut concurrent = ConcurrencyBuilder::new();

        // shutdown from all activities
        for e in &mut instance.activities {
            let invokes = e.stop.take().unwrap()().unwrap();
            concurrent.with_branch(invokes);
        }
        seq.with_step(concurrent.build(design));
        seq.with_step(TriggerBuilder::from_design(
            format!("{}/{}/shutdown_completed", app_name, instance.agent_name).as_str(),
            design,
        ));

        seq.build()
    }
}

struct InnerFeoLocal {
    activities: Vec<ActivityDetails>,
    agent_name: &'static str,
    app_name: &'static str,
    is_local: bool,
}

fn register_events(
    app_name: &str,
    agent_name: &str,
    activities: &Vec<ActivityDetails>,
    design: &mut Design,
) -> Result<(), CommonErrors> {
    info!("Registering events for agent: {}/{}", app_name, agent_name);

    design.register_event(format!("{}/{}/alive", app_name, agent_name).as_str().into())?;

    design.register_event(
        format!("{}/{}/startup", app_name, agent_name)
            .as_str()
            .into(),
    )?;

    design.register_event(
        format!("{}/{}/startup_completed", app_name, agent_name)
            .as_str()
            .into(),
    )?;
    design.register_event(
        format!("{}/{}/shutdown", app_name, agent_name)
            .as_str()
            .into(),
    )?;

    design.register_event(
        format!("{}/{}/shutdown_completed", app_name, agent_name)
            .as_str()
            .into(),
    )?;

    // register step events for activities
    for e in activities {
        design.register_event(format!("{}/{}/step", app_name, e.id).as_str().into())?;
        design.register_event(
            format!("{}/{}/step_completed", app_name, e.id)
                .as_str()
                .into(),
        )?;
    }

    Ok(())
}

struct InnerExecutor {
    agents: Vec<String>,
    graph: ActivityDependencies,
    app_name: &'static str,
    activity_to_agent: HashMap<ActivityId, &'static str>,
}
///
/// Responsible for controlling Task Chain execution across processes according to provided configuration
///
pub struct GlobalOrchestrator {
    inner: Rc<RefCell<InnerExecutor>>,
}

impl GlobalOrchestrator {
    pub fn new(
        app_name: &'static str,
        agents: Vec<String>,
        deps: ActivityDependencies,
        activity_to_agent: HashMap<ActivityId, &'static str>,
    ) -> Self {
        Self {
            inner: Rc::new(RefCell::new(InnerExecutor {
                agents: agents,
                graph: deps,
                app_name,
                activity_to_agent,
            })),
        }
    }

    fn get_orchestration_design(&self) -> Result<Design, CommonErrors> {
        let mut design = Design::new("global_orchestrator".into(), DesignConfig::default());
        let inner = self.inner.borrow();
        info!("Registering events for executor");

        for agent_name in &inner.agents {
            // register events for agent
            design.register_event(
                format!("{}/{}/startup", inner.app_name, agent_name)
                    .as_str()
                    .into(),
            )?;
            design.register_event(
                format!("{}/{}/shutdown", inner.app_name, agent_name)
                    .as_str()
                    .into(),
            )?;
            design.register_event(
                format!("{}/{}/alive", inner.app_name, agent_name)
                    .as_str()
                    .into(),
            )?;

            design.register_event(
                format!("{}/{}/startup_completed", inner.app_name, agent_name)
                    .as_str()
                    .into(),
            )?;

            design.register_event(
                format!("{}/{}/shutdown_completed", inner.app_name, agent_name)
                    .as_str()
                    .into(),
            )?;
        }

        for e in inner.graph.keys() {
            design.register_event(format!("{}/{}/step", inner.app_name, e).as_str().into())?;
            design.register_event(
                format!("{}/{}/step_completed", inner.app_name, e)
                    .as_str()
                    .into(),
            )?;
        }

        let inner_clone = self.inner.clone();
        // add program
        design.add_program(
            "global_orchestrator_program",
            move |design_instance, builder| {
                let borrowed = inner_clone.borrow_mut();

                builder
                    .with_start_action(borrowed.startup(design_instance))
                    .with_run_action(borrowed.generate_body(design_instance))
                    .with_stop_action(borrowed.shutdown(design_instance), Duration::from_secs(5));

                Ok(())
            },
        );

        Ok(design)
    }

    fn bind_events(
        &self,
        deployment: &mut Deployment,
        agent_name: &'static str,
    ) -> Result<(), CommonErrors> {
        self.inner.borrow_mut().bind_events(deployment, agent_name)
    }
}

impl InnerExecutor {
    fn sync_to_agents(&self, design: &mut Design) -> Box<dyn ActionTrait> {
        let mut top = ConcurrencyBuilder::new();

        for name in &self.agents {
            let sub_sequence = SyncBuilder::from_design(
                format!("{}/{}/alive", self.app_name, name).as_str(),
                design,
            );

            top.with_branch(sub_sequence);
        }

        top.build(design)
    }

    fn release_agents(&self, design: &mut Design) -> Box<dyn ActionTrait> {
        let mut top = SequenceBuilder::new();

        for name in &self.agents {
            let sub_sequence = TriggerBuilder::from_design(
                format!("{}/{}/startup", self.app_name, name).as_str(),
                design,
            );

            top.with_step(sub_sequence);
        }

        top.build()
    }

    fn wait_startup_completed(&self, design: &mut Design) -> Box<dyn ActionTrait> {
        let mut top = SequenceBuilder::new();

        for name in &self.agents {
            let sub_sequence = SyncBuilder::from_design(
                format!("{}/{}/startup_completed", self.app_name, name).as_str(),
                design,
            );

            top.with_step(sub_sequence);
        }

        top.build()
    }

    fn startup(&self, design: &mut Design) -> Box<dyn ActionTrait> {
        SequenceBuilder::new()
            .with_step(self.sync_to_agents(design))
            .with_step(self.release_agents(design))
            .with_step(self.wait_startup_completed(design))
            .build()
    }

    fn shutdown_agents(&self, design: &mut Design) -> Box<dyn ActionTrait> {
        let mut top = SequenceBuilder::new();

        for name in &self.agents {
            let sub_sequence = TriggerBuilder::from_design(
                format!("{}/{}/shutdown", self.app_name, name).as_str(),
                design,
            );

            top.with_step(sub_sequence);
        }

        top.build()
    }

    fn wait_shutdown_completed(&self, design: &mut Design) -> Box<dyn ActionTrait> {
        let mut top = SequenceBuilder::new();

        for name in &self.agents {
            let sub_sequence = SyncBuilder::from_design(
                format!("{}/{}/shutdown_completed", self.app_name, name).as_str(),
                design,
            );

            top.with_step(sub_sequence);
        }

        top.build()
    }

    fn shutdown(&self, design: &mut Design) -> Box<dyn ActionTrait> {
        SequenceBuilder::new()
            .with_step(self.shutdown_agents(design))
            .with_step(self.wait_shutdown_completed(design))
            .build()
    }

    // This can be used to stop orchestration from another application for demo.
    // fn orch_shutdown_notification(&self) -> Box<dyn ActionTrait> {
    //     let seq = Sequence::new_with_id(NamedId::new_static("shutdown"))
    //         .with_step(Sync::new("qorix_orch_shutdown_event"));

    //     seq
    // }

    // Converts a dependency graph into an execution sequence.
    fn generate_body(&self, design: &mut Design) -> Box<dyn ActionTrait> {
        let mut body = ConcurrencyBuilder::new();

        // For now simply mapping, without optimization
        for node in &self.graph {
            if node.1.is_empty() {
                body.with_branch(self.generate_step(design, &node.0));
                continue;
            }

            let mut s = SequenceBuilder::new();
            for dep in node.1 {
                s.with_step(SyncBuilder::from_design(
                    format!("{}/{}/step_completed", self.app_name, dep).as_str(),
                    design,
                ));
            }

            s.with_step(self.generate_step(design, &node.0));

            body.with_branch(s.build());
        }
        body.build(design)
    }

    fn generate_step(&self, design: &mut Design, id: &ActivityId) -> Box<dyn ActionTrait> {
        SequenceBuilder::new()
            .with_step(TriggerBuilder::from_design(
                format!("{}/{}/step", self.app_name, id).as_str(),
                design,
            ))
            .with_step(SyncBuilder::from_design(
                format!("{}/{}/step_completed", self.app_name, id).as_str(),
                design,
            ))
            .build()
    }

    /// This function binds only global events for the executor.
    /// The local events are bound by the agents themselves.
    fn bind_events(
        &self,
        deployment: &mut Deployment,
        agent_name: &'static str,
    ) -> Result<(), CommonErrors> {
        let mut bind_common_global_events = true;
        for e in self.graph.keys() {
            // Skip if there is no activity for the secondary agent
            if self.activity_to_agent.get(e).unwrap() != &agent_name {
                continue;
            }

            if bind_common_global_events {
                bind_helper(
                    format!("{}/{}/startup", self.app_name, agent_name),
                    deployment,
                    false,
                );

                bind_helper(
                    format!("{}/{}/shutdown", self.app_name, agent_name),
                    deployment,
                    false,
                );

                bind_helper(
                    format!("{}/{}/alive", self.app_name, agent_name),
                    deployment,
                    false,
                );

                bind_helper(
                    format!("{}/{}/startup_completed", self.app_name, agent_name),
                    deployment,
                    false,
                );

                bind_helper(
                    format!("{}/{}/shutdown_completed", self.app_name, agent_name),
                    deployment,
                    false,
                );

                bind_common_global_events = false;
            }

            bind_helper(format!("{}/{}/step", self.app_name, e), deployment, false);

            bind_helper(
                format!("{}/{}/step_completed", self.app_name, e),
                deployment,
                false,
            );
        }

        Ok(())
    }
}

fn bind_helper(event: String, deployment: &mut Deployment, is_local: bool) {
    let res = if is_local {
        deployment.bind_events_as_local(&[event.into()])
    } else {
        deployment.bind_events_as_global(event.as_str(), &[event.as_str().into()])
    };

    assert!(
        res.is_ok() || (res.unwrap_err() == CommonErrors::AlreadyDone),
        "Failed to bind event {:?}",
        res
    );
}
