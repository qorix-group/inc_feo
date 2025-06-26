use orchestration::actions::invoke::InvokeResult;

use std::sync::{Arc, Mutex};

use crate::ids::ActivityId;

pub trait ActivityAdapterTrait: Send {
    type T; // Activity Type

    ///
    /// This let you use async context in step function so You are free now to use non blocking sleep, non blocking wait on IO etc.
    /// There is no problem to create trait with plain `fn` but then async context is lost for activity
    ///
    fn step_runtime(
        instance: Arc<Mutex<Self::T>>,
    ) -> impl std::future::Future<Output = InvokeResult> + Send;

    fn start(&mut self) -> InvokeResult;

    fn stop(&mut self) -> InvokeResult;

    fn get_act_id(&self) -> ActivityId;
}
