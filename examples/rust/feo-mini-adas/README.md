# feo-mini-adas using Qorix FEO (qor-feo)

### 1. Introduction

The **qor-feo** provides necessary **executor** & **agent** to execute the **activities** across primary and secondary processes as per defined execution order. The developer shall use the interfaces provided by them and develop the application. The following sections explain the development steps.

### 2. Development of activity/component

- The developer shall implement `trait Activity` from /qor-feo/src/activity.rs for the **activity** in addition to other required functionalities.
- When the instance of the **activity** is built, it shall return self (not the Box).

### 3. Configuration

- The configuration related to TOPICS and its initialization shall be handled as usual.
- The configuration related to activity chain and agent shall be part of respective primary and secondary processes.

### 4. Application: Primary and Secondary Processes

**4.a. Primary Process**

- Initialize log, trace and TOPICS.
- Create string IDs for all the activity and agents (needed for event communication using iceoryx2).
- Define activitiy chain (execution_structure).
- Build instances of activities which are part of primary process and create an agent with them.
- Create an executor with name of activities, agents, execution engine, etc.
- Start the executor with activity chain.

**4.b. Secondary Process**

- Initialize log and trace.
- Build instances of activities which are part of secondary process and create an agent with them.
- Start the execution of agent.

### 5. Build and Run

Run in three terminals in the following order:

```sh
cargo run --bin adas_primary
```

```sh
cargo run --bin adas_secondary_1
```

```sh
cargo run --bin adas_secondary_2
```

## Notes:
1. The **qor-feo** depends on the *Runtime* provided by **qor-rto**. The **qor-rto** is generic and can be used in other application frameworks or application development. Please refer to **/qor-rto/docs/** for more details.

2. Please refer to **/feo-tracing/README.md** for tracing feo-mini-adas.