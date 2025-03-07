// Copyright 2025 Accenture.
//
// SPDX-License-Identifier: Apache-2.0

use crate::activities::messages::{BrakeInstruction, CameraImage, RadarScan, Scene, Steering};
use crate::ffi::{
    create_lane_assist, free_lane_assist, shutdown_lane_assist, startup_lane_assist,
    step_lane_assist,
};
use feo::com::{ActivityInput, ActivityOutput};
use feo_log::debug;
use feo_tracing::{instrument, tracing};
use qor_feo::prelude::{Activity, ActivityId};
use qor_rto::prelude::*;
use std::ffi::c_void;
use std::hash::{BuildHasher as _, Hasher as _, RandomState};
use std::mem::MaybeUninit;
use std::ops::Range;
use std::thread;
use std::time::Duration;

const SLEEP_RANGE: Range<i64> = 10..45;

/// Camera activity
///
/// This activity emulates a camera generating a [CameraImage].
#[derive(Debug)]
pub struct Camera {
    /// ID of the activity
    activity_id: ActivityId,
    /// Image output
    output_image: ActivityOutput<CameraImage>,

    // Local state for pseudo-random output generation
    num_people: usize,
    num_cars: usize,
    distance_obstacle: f64,
}

unsafe impl Send for Camera {} // Explicitly marking Camera as Send

impl Camera {
    pub fn build(activity_id: ActivityId, image_topic: &str) -> Camera {
        Self {
            activity_id,
            output_image: ActivityOutput::get(image_topic),
            num_people: 4,
            num_cars: 10,
            distance_obstacle: 40.0,
        }
    }

    fn get_image(&mut self) -> CameraImage {
        const PEOPLE_CHANGE_PROP: f64 = 0.8;
        const CAR_CHANGE_PROP: f64 = 0.8;
        const DISTANCE_CHANGE_PROP: f64 = 1.0;

        self.num_people = random_walk_integer(self.num_people, PEOPLE_CHANGE_PROP, 1);
        self.num_cars = random_walk_integer(self.num_people, CAR_CHANGE_PROP, 2);
        let sample = random_walk_float(self.distance_obstacle, DISTANCE_CHANGE_PROP, 5.0);
        self.distance_obstacle = sample.clamp(20.0, 50.0);

        CameraImage {
            num_people: self.num_people,
            num_cars: self.num_cars,
            distance_obstacle: self.distance_obstacle,
        }
    }
}

impl Activity for Camera {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    fn getname(&mut self) -> String {
        self.activity_id.to_string()
    }

    #[instrument(name = "Camera startup")]
    fn startup(&mut self) -> RoutineResult {
        debug!("Camera startup completed");
        Ok(())
    }

    #[instrument(name = "Camera")]
    fn step(&mut self) -> RoutineResult {
        debug!("Stepping Camera");
        sleep_random();

        if let Some(camera) = self.output_image.write_uninit() {
            let image = self.get_image();
            debug!("Sending image: {image:?}");
            let camera = camera.write_payload(image);
            camera.send();
        }
        Ok(())
    }

    #[instrument(name = "Camera shutdown")]
    fn shutdown(&mut self) -> RoutineResult {
        debug!("Camera shutdown completed");
        Ok(())
    }
}

/// Radar activity
///
/// This component emulates are radar generating a [RadarScan].
#[derive(Debug)]
pub struct Radar {
    /// ID of the activity
    activity_id: ActivityId,
    /// Radar scan output
    output_scan: ActivityOutput<RadarScan>,

    // Local state for pseudo-random output generation
    distance_obstacle: f64,
}

unsafe impl Send for Radar {} // Explicitly marking Radar as Send

impl Radar {
    pub fn build(activity_id: ActivityId, radar_topic: &str) -> Radar {
        Self {
            activity_id,
            output_scan: ActivityOutput::get(radar_topic),
            distance_obstacle: 40.0,
        }
    }

    fn get_scan(&mut self) -> RadarScan {
        const DISTANCE_CHANGE_PROP: f64 = 1.0;

        let sample = random_walk_float(self.distance_obstacle, DISTANCE_CHANGE_PROP, 6.0);
        self.distance_obstacle = sample.clamp(16.0, 60.0);

        let error_margin = gen_random_in_range(-10..10) as f64 / 10.0;

        RadarScan {
            distance_obstacle: self.distance_obstacle,
            error_margin,
        }
    }
}

impl Activity for Radar {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    fn getname(&mut self) -> String {
        self.activity_id.to_string()
    }

    #[instrument(name = "Radar startup")]
    fn startup(&mut self) -> RoutineResult {
        debug!("Radar startup completed");
        Ok(())
    }

    #[instrument(name = "Radar")]
    fn step(&mut self) -> RoutineResult {
        debug!("Stepping Radar");
        sleep_random();

        if let Some(radar) = self.output_scan.write_uninit() {
            let scan = self.get_scan();
            debug!("Sending scan: {scan:?}");
            let radar = radar.write_payload(scan);
            radar.send();
        }
        Ok(())
    }

    #[instrument(name = "Radar shutdown")]
    fn shutdown(&mut self) -> RoutineResult {
        debug!("Radar shutdown completed");
        Ok(())
    }
}

/// Neural network activity
///
/// This component emulates a neural network
/// pseudo-inferring a [Scene] output
/// from the provided [Camera] and [Radar] inputs.
#[derive(Debug)]
pub struct NeuralNet {
    /// ID of the activity
    activity_id: ActivityId,
    /// Image input
    input_image: ActivityInput<CameraImage>,
    /// Radar scan input
    input_scan: ActivityInput<RadarScan>,
    /// Scene output
    output_scene: ActivityOutput<Scene>,
}
unsafe impl Send for NeuralNet {} // Explicitly marking NeuralNet as Send

impl NeuralNet {
    pub fn build(
        activity_id: ActivityId,
        image_topic: &str,
        scan_topic: &str,
        scene_topic: &str,
    ) -> NeuralNet {
        Self {
            activity_id,
            input_image: ActivityInput::get(image_topic),
            input_scan: ActivityInput::get(scan_topic),
            output_scene: ActivityOutput::get(scene_topic),
        }
    }

    fn infer(image: &CameraImage, radar: &RadarScan, scene: &mut MaybeUninit<Scene>) {
        let CameraImage {
            num_people,
            num_cars,
            distance_obstacle,
        } = *image;

        let distance_obstacle = distance_obstacle.min(radar.distance_obstacle);
        let distance_left_lane = gen_random_in_range(5..10) as f64 / 10.0;
        let distance_right_lane = gen_random_in_range(5..10) as f64 / 10.0;

        // Get raw pointer to payload within `MaybeUninit`.
        let scene_ptr = scene.as_mut_ptr();

        // Safety: `scene_ptr` was create from a `MaybeUninit` of the right type and size.
        // The underlying type `Scene` has `repr(C)` and can be populated field by field.
        unsafe {
            (*scene_ptr).num_people = num_people;
            (*scene_ptr).num_cars = num_cars;
            (*scene_ptr).distance_obstacle = distance_obstacle;
            (*scene_ptr).distance_left_lane = distance_left_lane;
            (*scene_ptr).distance_right_lane = distance_right_lane;
        }
    }
}

impl Activity for NeuralNet {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    fn getname(&mut self) -> String {
        self.activity_id.to_string()
    }

    #[instrument(name = "NeuralNet startup")]
    fn startup(&mut self) -> RoutineResult {
        debug!("NeuralNet startup completed");
        Ok(())
    }

    #[instrument(name = "NeuralNet")]
    fn step(&mut self) -> RoutineResult {
        debug!("Stepping NeuralNet");
        sleep_random();

        let camera = self.input_image.read();
        let radar = self.input_scan.read();
        let scene = self.output_scene.write_uninit();

        if let (Some(camera), Some(radar), Some(mut scene)) = (camera, radar, scene) {
            debug!("Inferring scene with neural network");

            Self::infer(camera.get(), radar.get(), scene.payload_mut());
            // Safety: `Scene` has `repr(C)` and was fully initialized by `Self::infer` above.
            let scene = unsafe { scene.assume_init() };
            scene.send();
        }
        Ok(())
    }

    #[instrument(name = "NeuralNet shutdown")]
    fn shutdown(&mut self) -> RoutineResult {
        debug!("NeuralNet shutdown completed");
        Ok(())
    }
}

/// Emergency braking activity
///
/// This component emulates an emergency braking function
/// which sends instructions to activate the brakes
/// if the distance to the closest obstacle becomes too small.
/// The level of brake engagement depends on the distance.
#[derive(Debug)]
pub struct EmergencyBraking {
    /// ID of the activity
    activity_id: ActivityId,
    /// Scene input
    input_scene: ActivityInput<Scene>,
    /// Brake instruction output
    output_brake_instruction: ActivityOutput<BrakeInstruction>,
}

unsafe impl Send for EmergencyBraking {} // Explicitly marking EmergencyBraking as Send

impl EmergencyBraking {
    pub fn build(
        activity_id: ActivityId,
        scene_topic: &str,
        brake_instruction_topic: &str,
    ) -> EmergencyBraking {
        Self {
            activity_id,
            input_scene: ActivityInput::get(scene_topic),
            output_brake_instruction: ActivityOutput::get(brake_instruction_topic),
        }
    }
}

impl Activity for EmergencyBraking {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    fn getname(&mut self) -> String {
        self.activity_id.to_string()
    }

    #[instrument(name = "EmergencyBraking startup")]
    fn startup(&mut self) -> RoutineResult {
        debug!("EmergencyBraking startup completed");
        Ok(())
    }

    #[instrument(name = "EmergencyBraking")]
    fn step(&mut self) -> RoutineResult {
        debug!("Stepping EmergencyBraking");
        sleep_random();

        let scene = self.input_scene.read();
        let brake_instruction = self.output_brake_instruction.write_uninit();

        if let (Some(scene), Some(brake_instruction)) = (scene, brake_instruction) {
            const ENGAGE_DISTANCE: f64 = 30.0;
            const MAX_BRAKE_DISTANCE: f64 = 15.0;

            if scene.get().distance_obstacle < ENGAGE_DISTANCE {
                // Map distances ENGAGE_DISTANCE..MAX_BRAKE_DISTANCE to intensities 0.0..1.0
                let level = f64::min(
                    1.0,
                    (ENGAGE_DISTANCE - scene.get().distance_obstacle)
                        / (ENGAGE_DISTANCE - MAX_BRAKE_DISTANCE),
                );

                let brake_instruction = brake_instruction.write_payload(BrakeInstruction {
                    active: true,
                    level,
                });
                brake_instruction.send();
            } else {
                let brake_instruction = brake_instruction.write_payload(BrakeInstruction {
                    active: false,
                    level: 0.0,
                });
                brake_instruction.send();
            }
        }
        Ok(())
    }

    #[instrument(name = "EmergencyBraking shutdown")]
    fn shutdown(&mut self) -> RoutineResult {
        debug!("EmergencyBraking shutdown completed");
        Ok(())
    }
}

/// Brake controller activity
///
/// This component emulates a brake controller
/// which triggers the brakes based on an instruction
/// and therefore might run in a separate process
/// with only other ASIL-D activities.
#[derive(Debug)]
pub struct BrakeController {
    /// ID of the activity
    activity_id: ActivityId,
    /// Brake instruction input
    input_brake_instruction: ActivityInput<BrakeInstruction>,
}

impl BrakeController {
    pub fn build(activity_id: ActivityId, brake_instruction_topic: &str) -> BrakeController {
        Self {
            activity_id,
            input_brake_instruction: ActivityInput::get(brake_instruction_topic),
        }
    }
}

unsafe impl Send for BrakeController {} // Explicitly marking BrakeController as Send

impl Activity for BrakeController {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    fn getname(&mut self) -> String {
        self.activity_id.to_string()
    }

    #[instrument(name = "BrakeController startup")]
    fn startup(&mut self) -> RoutineResult {
        debug!("BrakeController startup completed");
        Ok(())
    }

    #[instrument(name = "BrakeController")]
    fn step(&mut self) -> RoutineResult {
        debug!("Stepping BrakeController");
        sleep_random();

        if let Some(brake_instruction) = self.input_brake_instruction.read() {
            if brake_instruction.get().active {
                debug!(
                    "BrakeController activating brakes with level {:.3}",
                    brake_instruction.get().level
                )
            }
        }
        Ok(())
    }

    #[instrument(name = "BrakeController shutdown")]
    fn shutdown(&mut self) -> RoutineResult {
        debug!("BrakeController shutdown completed");
        Ok(())
    }
}

/// Environment renderer activity
///
/// This component emulates a renderer to display a scene
/// in the infotainment display.
/// In this example, it does not do anything with the scene input.
#[derive(Debug)]
pub struct EnvironmentRenderer {
    /// ID of the activity
    activity_id: ActivityId,
    /// Scene input
    input_scene: ActivityInput<Scene>,
}

impl EnvironmentRenderer {
    pub fn build(activity_id: ActivityId, scene_topic: &str) -> EnvironmentRenderer {
        Self {
            activity_id,
            input_scene: ActivityInput::get(scene_topic),
        }
    }
}

unsafe impl Send for EnvironmentRenderer {} // Explicitly marking EnvironmentRenderer as Send

impl Activity for EnvironmentRenderer {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    fn getname(&mut self) -> String {
        self.activity_id.to_string()
    }

    #[instrument(name = "EnvironmentRenderer startup")]
    fn startup(&mut self) -> RoutineResult {
        debug!("EnvironmentRenderer startup completed");
        Ok(())
    }

    #[instrument(name = "EnvironmentRenderer")]
    fn step(&mut self) -> RoutineResult {
        debug!("Stepping EnvironmentRenderer");
        sleep_random();

        if let Some(_scene) = self.input_scene.read() {
            debug!("Rendering scene");
        }
        Ok(())
    }

    #[instrument(name = "EnvironmentRenderer shutdown")]
    fn shutdown(&mut self) -> RoutineResult {
        debug!("EnvironmentRenderer shutdown completed");
        Ok(())
    }
}

/// Lane assistant activity
///
/// This component emulates a lane assistant function
/// which sends steering instructions to change the car heading.
/// The steering angle depends on the distance to the left and right lanes.
/// This is a wrapper around a C++ component implementing the example logic.
#[derive(Debug)]
pub struct LaneAssist {
    /// ID of the activity
    activity_id: ActivityId,
    /// Scene input
    input_scene: ActivityInput<Scene>,
    /// Steering output
    output_steering: ActivityOutput<Steering>,
    /// Pointer to wrapped initialized C++ class instance
    cpp_activity: *mut c_void,
}

unsafe impl Send for LaneAssist {} // Explicitly marking LaneAssist as Send

impl LaneAssist {
    pub fn build(activity_id: ActivityId, scene_topic: &str, steering_topic: &str) -> LaneAssist {
        // Create C++ activity in heap memory of C++
        let cpp_activity = unsafe { create_lane_assist(activity_id.into()) };

        Self {
            activity_id,
            input_scene: ActivityInput::get(scene_topic),
            output_steering: ActivityOutput::get(steering_topic),
            cpp_activity,
        }
    }
}

impl Drop for LaneAssist {
    fn drop(&mut self) {
        // Free C++ activity in heap memory of C++
        unsafe { free_lane_assist(self.cpp_activity) };
    }
}

impl Activity for LaneAssist {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    fn getname(&mut self) -> String {
        self.activity_id.to_string()
    }

    #[instrument(name = "LaneAssist startup")]
    fn startup(&mut self) -> RoutineResult {
        unsafe { startup_lane_assist(self.cpp_activity) };
        debug!("LaneAssist startup completed");
        Ok(())
    }

    #[instrument(name = "LaneAssist")]
    fn step(&mut self) -> RoutineResult {
        debug!("Stepping LaneAssist");
        sleep_random();

        let scene = self.input_scene.read();
        let steering = self.output_steering.write_init();

        if let (Some(scene), Some(mut steering)) = (scene, steering) {
            // Call C++ activity with references to input and output
            unsafe { step_lane_assist(self.cpp_activity, scene.get(), steering.get_mut()) };

            debug!(
                "Steering angle in LaneAssist Output: {:?}",
                steering.get_mut()
            );

            steering.send();
        }
        Ok(())
    }

    #[instrument(name = "LaneAssist shutdown")]
    fn shutdown(&mut self) -> RoutineResult {
        unsafe { shutdown_lane_assist(self.cpp_activity) };
        debug!("LaneAssist shutdown completed");
        Ok(())
    }
}

/// Steering controller activity
///
/// This component emulates a steering controller
/// which adjusts the steering angle to control the heading of the car.
/// Therefore, it might run in a separate process
/// with only other ASIL-D activities.
#[derive(Debug)]
pub struct SteeringController {
    /// ID of the activity
    activity_id: ActivityId,
    /// Steering input
    input_steering: ActivityInput<Steering>,
}

unsafe impl Send for SteeringController {} // Explicitly marking SteeringController as Send

impl SteeringController {
    pub fn build(activity_id: ActivityId, steering_topic: &str) -> SteeringController {
        Self {
            activity_id,
            input_steering: ActivityInput::get(steering_topic),
        }
    }
}

impl Activity for SteeringController {
    fn id(&self) -> ActivityId {
        self.activity_id
    }

    fn getname(&mut self) -> String {
        self.activity_id.to_string()
    }

    #[instrument(name = "SteeringController startup")]
    fn startup(&mut self) -> RoutineResult {
        debug!("SteeringController startup completed");
        Ok(())
    }

    #[instrument(name = "SteeringController")]
    fn step(&mut self) -> RoutineResult {
        debug!("Stepping SteeringController");
        sleep_random();

        if let Some(steering) = self.input_steering.read() {
            debug!(
                "SteeringController adjusting angle to {:.3}",
                steering.get().angle
            )
        }
        Ok(())
    }

    #[instrument(name = "SteeringController shutdown")]
    fn shutdown(&mut self) -> RoutineResult {
        debug!("SteeringController shutdown completed");
        Ok(())
    }
}

/// Generate a pseudo-random number in the specified range.
fn gen_random_in_range(range: Range<i64>) -> i64 {
    let rand = RandomState::new().build_hasher().finish();
    let rand = (rand % (i64::MAX as u64)) as i64;
    rand % (range.end - range.start + 1) + range.start
}

/// Random walk from `previous` with a probability of `change_prop` in a range of +/-`max_delta`
fn random_walk_float(previous: f64, change_prop: f64, max_delta: f64) -> f64 {
    if gen_random_in_range(0..100) as f64 / 100.0 < change_prop {
        const SCALE_FACTOR: f64 = 1000.0;

        // Scale delta to work in integers
        let scaled_max_delta = (max_delta * SCALE_FACTOR) as i64;
        let scaled_delta = gen_random_in_range(-scaled_max_delta..scaled_max_delta) as f64;

        return previous + (scaled_delta / SCALE_FACTOR);
    }

    previous
}

/// Random walk from `previous` with a probability of `change_prop` in a range of +/-`max_delta`
fn random_walk_integer(previous: usize, change_prop: f64, max_delta: usize) -> usize {
    let max_delta = max_delta as i64;

    if gen_random_in_range(0..100) as f64 / 100.0 < change_prop {
        let delta = gen_random_in_range(-max_delta..max_delta);

        return i64::max(0, previous as i64 + delta) as usize;
    }

    previous
}

/// Sleep for a random amount of time
fn sleep_random() {
    thread::sleep(Duration::from_millis(
        gen_random_in_range(SLEEP_RANGE) as u64
    ));
}
