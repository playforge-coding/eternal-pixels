// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Stroke input: the sampled events a stroke is generated from.

use core::time::Duration;

use crate::error::{Result, check};
use crate::ffi;
use crate::geometry::Point;

/// Ink spells "this optional field is absent" with sentinel values rather than
/// an optional type. Zero means absent for a physical length, -1 for pressure
/// and for each of the stylus angles.
const ABSENT_LENGTH_CM: f32 = 0.0;
const ABSENT: f32 = -1.0;

fn to_sentinel(value: Option<f32>, absent: f32) -> f32 {
    value.unwrap_or(absent)
}

fn from_sentinel(value: f32, absent: f32) -> Option<f32> {
    if value == absent { None } else { Some(value) }
}

/// What produced an input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ToolType {
    #[default]
    Unknown,
    Mouse,
    Touch,
    Stylus,
}

impl ToolType {
    fn to_ffi(self) -> ffi::ToolType {
        match self {
            Self::Unknown => ffi::ToolType::kUnknown,
            Self::Mouse => ffi::ToolType::kMouse,
            Self::Touch => ffi::ToolType::kTouch,
            Self::Stylus => ffi::ToolType::kStylus,
        }
    }

    fn from_ffi(tool_type: ffi::ToolType) -> Self {
        match tool_type {
            t if t == ffi::ToolType::kMouse => Self::Mouse,
            t if t == ffi::ToolType::kTouch => Self::Touch,
            t if t == ffi::ToolType::kStylus => Self::Stylus,
            _ => Self::Unknown,
        }
    }
}

/// One sampled input event.
///
/// Every field but `tool_type`, `position` and `elapsed_time` is optional, and
/// all the inputs in a batch have to agree on which ones are present. Mixing,
/// say, some events with pressure and some without is rejected when the batch
/// is built.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StrokeInput {
    /// What produced the event.
    pub tool_type: ToolType,
    /// Where it happened, in stroke units.
    pub position: Point,
    /// Time since the start of the stroke.
    pub elapsed_time: Duration,
    /// How long one stroke unit is, in centimetres. Needed by brush behaviours
    /// that work in physical units.
    pub stroke_unit_length_cm: Option<f32>,
    /// Stylus or touch pressure, normally in `0.0..=1.0`.
    pub pressure: Option<f32>,
    /// Stylus tilt away from perpendicular, in radians.
    pub tilt: Option<f32>,
    /// Direction the stylus is tilted towards, in radians.
    pub orientation: Option<f32>,
    /// Rotation of the stylus about its own axis, in radians.
    pub barrel_twist: Option<f32>,
}

impl StrokeInput {
    /// The minimum an input needs: where and when.
    pub fn new(tool_type: ToolType, position: Point, elapsed_time: Duration) -> Self {
        Self {
            tool_type,
            position,
            elapsed_time,
            ..Default::default()
        }
    }

    /// Adds pressure to an input.
    pub fn with_pressure(mut self, pressure: f32) -> Self {
        self.pressure = Some(pressure);
        self
    }

    /// Adds stylus tilt and orientation, both in radians.
    pub fn with_tilt_orientation(mut self, tilt: f32, orientation: f32) -> Self {
        self.tilt = Some(tilt);
        self.orientation = Some(orientation);
        self
    }

    pub(crate) fn to_ffi(self) -> ffi::StrokeInput {
        ffi::StrokeInput {
            tool_type: self.tool_type.to_ffi(),
            position: self.position.to_ffi(),
            elapsed_time_seconds: self.elapsed_time.as_secs_f32(),
            stroke_unit_length_cm: to_sentinel(self.stroke_unit_length_cm, ABSENT_LENGTH_CM),
            pressure: to_sentinel(self.pressure, ABSENT),
            tilt_radians: to_sentinel(self.tilt, ABSENT),
            orientation_radians: to_sentinel(self.orientation, ABSENT),
            barrel_twist_radians: to_sentinel(self.barrel_twist, ABSENT),
        }
    }

    pub(crate) fn from_ffi(input: ffi::StrokeInput) -> Self {
        Self {
            tool_type: ToolType::from_ffi(input.tool_type),
            position: Point::from_ffi(input.position),
            elapsed_time: Duration::try_from_secs_f32(input.elapsed_time_seconds)
                .unwrap_or(Duration::ZERO),
            stroke_unit_length_cm: from_sentinel(input.stroke_unit_length_cm, ABSENT_LENGTH_CM),
            pressure: from_sentinel(input.pressure, ABSENT),
            tilt: from_sentinel(input.tilt_radians, ABSENT),
            orientation: from_sentinel(input.orientation_radians, ABSENT),
            barrel_twist: from_sentinel(input.barrel_twist_radians, ABSENT),
        }
    }
}

/// An ordered run of [`StrokeInput`]s.
///
/// Ink validates the whole batch up front: times must not go backwards, every
/// input must report the same tool type, and the optional fields must be
/// present on all of them or none.
pub struct StrokeInputBatch {
    handle: ffi::StrokeInputBatchHandle,
}

impl StrokeInputBatch {
    /// Builds a batch from a slice of inputs.
    pub fn new(inputs: &[StrokeInput]) -> Result<Self> {
        Self::with_seed(inputs, 0, 0.0)
    }

    /// Builds a batch, choosing the seed for brushes with randomised noise and
    /// the starting phase for animated brush paints.
    pub fn with_seed(
        inputs: &[StrokeInput],
        noise_seed: u32,
        base_paint_animation_phase: f32,
    ) -> Result<Self> {
        let converted: Vec<ffi::StrokeInput> = inputs.iter().map(|i| i.to_ffi()).collect();
        let count = i32::try_from(converted.len()).map_err(|_| {
            crate::error::Error::new(
                crate::error::ErrorKind::InvalidArgument,
                "too many stroke inputs",
            )
        })?;

        let mut out = ffi::StrokeInputBatchHandle {
            handle: core::ptr::null_mut(),
        };
        let status = unsafe {
            ffi::StrokeInputBatchCreate(
                converted.as_ptr(),
                count,
                noise_seed,
                base_paint_animation_phase,
                &mut out,
            )
        };
        check(status)?;
        Ok(Self::from_ffi(out))
    }

    /// An empty batch.
    pub fn empty() -> Self {
        Self::new(&[]).expect("an empty stroke input batch is always valid")
    }

    pub(crate) fn as_ffi(&self) -> ffi::StrokeInputBatchHandle {
        self.handle
    }

    pub(crate) fn from_ffi(handle: ffi::StrokeInputBatchHandle) -> Self {
        assert!(
            !handle.handle.is_null(),
            "ink: failed to allocate stroke input batch"
        );
        Self { handle }
    }

    pub fn len(&self) -> usize {
        usize::try_from(unsafe { ffi::StrokeInputBatchSize(self.handle) }).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The input at `index`, or `None` if it is past the end.
    pub fn get(&self, index: usize) -> Option<StrokeInput> {
        if index >= self.len() {
            return None;
        }
        let index = i32::try_from(index).ok()?;
        Some(StrokeInput::from_ffi(unsafe {
            ffi::StrokeInputBatchGet(self.handle, index)
        }))
    }

    /// Appends one input, which must be consistent with what is already here.
    pub fn push(&mut self, input: StrokeInput) -> Result<()> {
        check(unsafe { ffi::StrokeInputBatchAppend(self.handle, input.to_ffi()) })
    }

    /// Appends another batch.
    pub fn extend_from_batch(&mut self, other: &StrokeInputBatch) -> Result<()> {
        check(unsafe { ffi::StrokeInputBatchAppendBatch(self.handle, other.as_ffi()) })
    }

    pub fn clear(&mut self) {
        unsafe { ffi::StrokeInputBatchClear(self.handle) };
    }

    /// Time from the first input to the last.
    ///
    /// Ink keeps times as `f32` seconds, so this does not round-trip exactly:
    /// a batch built from 112ms of input reads back as 112.000003ms. Compare
    /// with a tolerance rather than for equality.
    pub fn duration(&self) -> Duration {
        let seconds = unsafe { ffi::StrokeInputBatchDurationSeconds(self.handle) };
        Duration::try_from_secs_f32(seconds).unwrap_or(Duration::ZERO)
    }

    /// The tool every input in the batch reports.
    pub fn tool_type(&self) -> ToolType {
        ToolType::from_ffi(unsafe { ffi::StrokeInputBatchToolType(self.handle) })
    }

    /// The seed used by brushes with randomised behaviour.
    pub fn noise_seed(&self) -> u32 {
        unsafe { ffi::StrokeInputBatchNoiseSeed(self.handle) }
    }

    /// Iterates over the inputs, copying each one out as it goes.
    pub fn iter(&self) -> impl Iterator<Item = StrokeInput> + '_ {
        (0..self.len()).filter_map(|i| self.get(i))
    }
}

impl Clone for StrokeInputBatch {
    fn clone(&self) -> Self {
        Self::from_ffi(unsafe { ffi::StrokeInputBatchClone(self.handle) })
    }
}

impl Drop for StrokeInputBatch {
    fn drop(&mut self) {
        unsafe { ffi::StrokeInputBatchDestroy(self.handle) };
    }
}

impl core::fmt::Debug for StrokeInputBatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StrokeInputBatch")
            .field("len", &self.len())
            .field("tool_type", &self.tool_type())
            .field("duration", &self.duration())
            .finish()
    }
}

impl TryFrom<&[StrokeInput]> for StrokeInputBatch {
    type Error = crate::error::Error;

    fn try_from(inputs: &[StrokeInput]) -> Result<Self> {
        Self::new(inputs)
    }
}
