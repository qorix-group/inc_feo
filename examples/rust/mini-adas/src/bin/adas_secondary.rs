// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(any(feature = "signalling_direct_tcp", feature = "signalling_direct_unix"))]
fn main() {
    use core::time::Duration;
    use feo::agent::com_init::initialize_com_secondary;
    use feo::agent::direct::secondary::{Secondary, SecondaryConfig};
    use feo::agent::NodeAddress;
    use feo::ids::ActivityId;
    use feo_log::{info, LevelFilter};
    #[cfg(feature = "signalling_direct_unix")]
    use mini_adas::config::socket_paths;
    #[cfg(feature = "signalling_direct_tcp")]
    use mini_adas::config::BIND_ADDR;
    use mini_adas::config::{agent_assignments, topic_dependencies};
    use mini_adas::config::{agent_assignments_ids, COM_BACKEND};
    use params::Params;
    use std::collections::HashSet;

    feo_logger::init(LevelFilter::Debug, true, true);
    feo_tracing::init(feo_tracing::LevelFilter::TRACE);

    let params = Params::from_args();
    info!("Starting agent {}", params.agent_id);

    let config = SecondaryConfig {
        id: params.agent_id,
        worker_assignments: agent_assignments().remove(&params.agent_id).unwrap(),
        timeout: Duration::from_secs(1),
        #[cfg(feature = "signalling_direct_tcp")]
        endpoint: NodeAddress::Tcp(BIND_ADDR),
        #[cfg(feature = "signalling_direct_unix")]
        endpoint: NodeAddress::UnixSocket(socket_paths().0),
    };

    // determine set of activity ids belonging to this agent
    let local_activities: HashSet<ActivityId> = agent_assignments_ids()
        .remove(&params.agent_id)
        .unwrap()
        .iter()
        .flat_map(|(_, acts)| acts.iter())
        .copied()
        .collect();

    // Initialize topics. Do not drop.
    let _topic_guards =
        initialize_com_secondary(COM_BACKEND, topic_dependencies(), &local_activities);

    let secondary = Secondary::new(config);
    secondary.run();
}

#[cfg(feature = "signalling_relayed_tcp")]
fn main() {
    use async_runtime::runtime::async_runtime::AsyncRuntimeBuilder;
    use async_runtime::scheduler::execution_engine::ExecutionEngineBuilder;
    use core::time::Duration;
    use feo::agent::com_init::initialize_com_secondary;
    use feo::agent::relayed::secondary::SecondaryConfig;
    use feo::agent::NodeAddress;
    use feo::ids::ActivityId;
    use feo::orch_adapter::runtime_adapters::FeoRunner;
    use feo_log::{info, LevelFilter};
    use logging_tracing::{TraceScope, TracingLibraryBuilder};

    use mini_adas::config::{agent_assignments, topic_dependencies, APPLICATION_NAME};
    use mini_adas::config::{agent_assignments_ids, COM_BACKEND};
    use mini_adas::config::{BIND_ADDR, BIND_ADDR2};
    use params::Params;
    use std::collections::HashSet;

    let params = Params::from_args();
    info!("Starting agent {}", params.agent_id);

    let _config = SecondaryConfig {
        id: params.agent_id,
        worker_assignments: agent_assignments().remove(&params.agent_id).unwrap(),
        timeout: Duration::from_secs(10),
        bind_address_senders: NodeAddress::Tcp(BIND_ADDR),
        bind_address_receivers: NodeAddress::Tcp(BIND_ADDR2),
    };

    // determine set of activity ids belonging to this agent
    let local_activities: HashSet<ActivityId> = agent_assignments_ids()
        .remove(&params.agent_id)
        .unwrap()
        .iter()
        .flat_map(|(_, acts)| acts.iter())
        .copied()
        .collect();

    // Initialize topics. Do not drop.
    let _topic_guards =
        initialize_com_secondary(COM_BACKEND, topic_dependencies(), &local_activities);

    use feo_tracing::Level;

    let mut logger = TracingLibraryBuilder::new()
        .global_log_level(Level::TRACE)
        .enable_tracing(TraceScope::SystemScope)
        .enable_logging(true)
        .build();

    logger.init_log_trace();

    let (builder, _engine_id) = AsyncRuntimeBuilder::new().with_engine(
        ExecutionEngineBuilder::new()
            .task_queue_size(256)
            .workers(2),
    );

    let mut runtime = builder.build().unwrap();
    let mut runner = FeoRunner::new(APPLICATION_NAME);

    if params.agent_id == params::SECONDARY_IDS[0] {
        use mini_adas::{activities::secondary1_agent_activities, config::SECONDARY1_NAME};

        runner.add_agent(secondary1_agent_activities(), SECONDARY1_NAME);
    } else if params.agent_id == params::SECONDARY_IDS[1] {
        use mini_adas::{activities::secondary2_agent_activities, config::SECONDARY2_NAME};

        runner.add_agent(secondary2_agent_activities(), SECONDARY2_NAME);
    } else {
        panic!("Invalid agent ID: {}", params.agent_id);
    }

    runner.run(&mut runtime, Duration::ZERO);
}

#[cfg(feature = "signalling_relayed_unix")]
fn main() {
    use core::time::Duration;
    use feo::agent::com_init::initialize_com_secondary;
    use feo::agent::relayed::secondary::{Secondary, SecondaryConfig};
    use feo::agent::NodeAddress;
    use feo::ids::ActivityId;
    use feo_log::{info, LevelFilter};
    use mini_adas::config::socket_paths;
    use mini_adas::config::{agent_assignments, topic_dependencies};
    use mini_adas::config::{agent_assignments_ids, COM_BACKEND};
    use params::Params;
    use std::collections::HashSet;

    feo_logger::init(LevelFilter::Debug, true, true);
    feo_tracing::init(feo_tracing::LevelFilter::TRACE);

    let params = Params::from_args();
    info!("Starting agent {}", params.agent_id);

    let config = SecondaryConfig {
        id: params.agent_id,
        worker_assignments: agent_assignments().remove(&params.agent_id).unwrap(),
        timeout: Duration::from_secs(10),
        bind_address_senders: NodeAddress::UnixSocket(socket_paths().0),
        bind_address_receivers: NodeAddress::UnixSocket(socket_paths().1),
    };

    // determine set of activity ids belonging to this agent
    let local_activities: HashSet<ActivityId> = agent_assignments_ids()
        .remove(&params.agent_id)
        .unwrap()
        .iter()
        .flat_map(|(_, acts)| acts.iter())
        .copied()
        .collect();

    // Initialize topics. Do not drop.
    let _topic_guards =
        initialize_com_secondary(COM_BACKEND, topic_dependencies(), &local_activities);

    let secondary = Secondary::new(config);
    secondary.run();
}

#[cfg(feature = "signalling_direct_mpsc")]
fn main() {
    panic!("Secondaries are not supported with this feature flag");
}

#[cfg(not(feature = "signalling_direct_mpsc"))]
mod params {
    use feo::ids::AgentId;
    /// Parameters of the secondary
    pub struct Params {
        /// Secondary agent ID
        pub agent_id: AgentId,
    }

    pub const SECONDARY_IDS: [AgentId; 2] = [AgentId::new(101), AgentId::new(102)];

    impl Params {
        pub fn from_args() -> Self {
            let args: Vec<String> = std::env::args().collect();

            let secondary_index = args
                .get(1)
                .and_then(|x| x.parse::<usize>().ok())
                .unwrap_or(1);

            let agent_id = SECONDARY_IDS
                .get(secondary_index - 1)
                .cloned()
                .unwrap_or_else(|| {
                    panic!(
                        "secondary index must be in the range 1 ... {}",
                        SECONDARY_IDS.len()
                    )
                });

            Self { agent_id }
        }
    }
}
