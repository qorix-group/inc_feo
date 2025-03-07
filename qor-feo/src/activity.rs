// Define the Activity trait

use qor_rto::prelude::*;

use std::fmt::Debug;

use std::fmt::Display;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActivityId(usize);
impl From<usize> for ActivityId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<&ActivityId> for usize {
    fn from(value: &ActivityId) -> Self {
        value.0
    }
}

impl From<ActivityId> for usize {
    fn from(value: ActivityId) -> Self {
        value.0
    }
}

impl Display for ActivityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait Activity: Send {
    fn startup(&mut self) -> RoutineResult;
    fn step(&mut self) -> RoutineResult;
    fn shutdown(&mut self) -> RoutineResult;
    fn getname(&mut self) -> String;
    fn id(&self) -> ActivityId;
}
