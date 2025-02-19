// Define the Activity trait

use qor_rto::prelude::*;

use std::fmt::Debug;

use std::marker::Sync;

pub trait Activity: Send + Sync{
    fn init(&mut self) -> RoutineResult;
    fn step(&mut self) -> RoutineResult;
    fn terminate(&mut self) -> RoutineResult;
    fn getname(&mut self)-> String;
}
