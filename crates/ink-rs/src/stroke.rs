// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Strokes: building them incrementally, and reading their geometry back.

use core::time::Duration;

use crate::brush::Brush;
use crate::error::{Result, check};
use crate::ffi;
use crate::geometry::{Mesh, Point, Rect};
use crate::input::StrokeInputBatch;

/// Pulls `count` vertex positions out of C++ via `copy`.
fn read_positions(count: u32, copy: impl FnOnce(*mut ffi::Point, i32) -> i32) -> Vec<Point> {
    let capacity = usize::try_from(count).unwrap_or(0);
    if capacity == 0 {
        return Vec::new();
    }

    let mut raw: Vec<ffi::Point> = Vec::with_capacity(capacity);
    let written = copy(
        raw.as_mut_ptr(),
        i32::try_from(capacity).unwrap_or(i32::MAX),
    );
    let written = usize::try_from(written).unwrap_or(0).min(capacity);
    // SAFETY: the facade wrote exactly `written` elements into the allocation,
    // and `written <= capacity`.
    unsafe { raw.set_len(written) };

    raw.into_iter().map(Point::from_ffi).collect()
}

/// Pulls `triangle_count * 3` indices out of C++ via `copy`.
fn read_indices(triangle_count: u32, copy: impl FnOnce(*mut u32, i32) -> i32) -> Vec<u32> {
    let capacity = usize::try_from(triangle_count)
        .unwrap_or(0)
        .saturating_mul(3);
    if capacity == 0 {
        return Vec::new();
    }

    let mut raw: Vec<u32> = Vec::with_capacity(capacity);
    let written = copy(
        raw.as_mut_ptr(),
        i32::try_from(capacity).unwrap_or(i32::MAX),
    );
    let written = usize::try_from(written).unwrap_or(0).min(capacity);
    // SAFETY: as above.
    unsafe { raw.set_len(written) };

    raw
}

/// A stroke being drawn.
///
/// The usual loop is [`start`](Self::start), then
/// [`enqueue_inputs`](Self::enqueue_inputs) and
/// [`update_shape`](Self::update_shape) as events arrive and frames are drawn,
/// then [`finish_inputs`](Self::finish_inputs) when the user lifts the pen and
/// [`to_stroke`](Self::to_stroke) to keep the result.
///
/// Geometry is read back per brush coat with [`coat_mesh`](Self::coat_mesh).
/// A brush with several coats produces one mesh per coat, drawn in order.
pub struct InProgressStroke {
    handle: ffi::InProgressStrokeHandle,
}

impl InProgressStroke {
    pub fn new() -> Self {
        let handle = unsafe { ffi::InProgressStrokeCreate() };
        assert!(
            !handle.handle.is_null(),
            "ink: failed to allocate in-progress stroke"
        );
        Self { handle }
    }

    /// Throws away any stroke in progress.
    pub fn clear(&mut self) {
        unsafe { ffi::InProgressStrokeClear(self.handle) };
    }

    /// Begins a stroke with `brush`, discarding whatever was in progress.
    pub fn start(&mut self, brush: &Brush) {
        self.start_with_seed(brush, 0, 0.0);
    }

    /// Begins a stroke, choosing the noise seed and the starting phase for
    /// animated brush paints.
    pub fn start_with_seed(&mut self, brush: &Brush, noise_seed: u32, base_animation_phase: f32) {
        unsafe {
            ffi::InProgressStrokeStart(
                self.handle,
                brush.as_ffi(),
                noise_seed,
                base_animation_phase,
            )
        };
    }

    /// Queues newly arrived input.
    ///
    /// `predicted` is input the platform expects but has not confirmed, used to
    /// hide latency; it is replaced wholesale on the next call rather than
    /// accumulated. Pass `None` when there is no prediction.
    pub fn enqueue_inputs(
        &mut self,
        real: &StrokeInputBatch,
        predicted: Option<&StrokeInputBatch>,
    ) -> Result<()> {
        let predicted = predicted.map(StrokeInputBatch::as_ffi).unwrap_or({
            ffi::StrokeInputBatchHandle {
                handle: core::ptr::null_mut(),
            }
        });
        check(unsafe { ffi::InProgressStrokeEnqueueInputs(self.handle, real.as_ffi(), predicted) })
    }

    /// Says no more input is coming.
    pub fn finish_inputs(&mut self) {
        unsafe { ffi::InProgressStrokeFinishInputs(self.handle) };
    }

    /// Regenerates geometry up to `elapsed`, measured from the start of the
    /// stroke. Must not go backwards between calls.
    pub fn update_shape(&mut self, elapsed: Duration) -> Result<()> {
        check(unsafe { ffi::InProgressStrokeUpdateShape(self.handle, elapsed.as_secs_f32()) })
    }

    /// Whether queued input or time-varying behaviour means the geometry is
    /// stale.
    pub fn needs_update(&self) -> bool {
        unsafe { ffi::InProgressStrokeNeedsUpdate(self.handle) }
    }

    pub fn inputs_are_finished(&self) -> bool {
        unsafe { ffi::InProgressStrokeInputsAreFinished(self.handle) }
    }

    /// Whether the brush keeps changing the shape as time passes, which means
    /// [`update_shape`](Self::update_shape) has to keep being called even after
    /// input stops.
    pub fn changes_with_time(&self) -> bool {
        unsafe { ffi::InProgressStrokeChangesWithTime(self.handle) }
    }

    /// Number of coats the current brush paints, and so the number of meshes.
    pub fn coat_count(&self) -> u32 {
        unsafe { ffi::InProgressStrokeBrushCoatCount(self.handle) }
    }

    /// Inputs processed so far, real and predicted.
    pub fn input_count(&self) -> usize {
        usize::try_from(unsafe { ffi::InProgressStrokeInputCount(self.handle) }).unwrap_or(0)
    }

    /// Of those, how many were real rather than predicted.
    pub fn real_input_count(&self) -> usize {
        usize::try_from(unsafe { ffi::InProgressStrokeRealInputCount(self.handle) }).unwrap_or(0)
    }

    /// Current geometry for one coat. An out-of-range coat gives an empty mesh.
    pub fn coat_mesh(&self, coat_index: u32) -> Mesh {
        let vertices = unsafe { ffi::InProgressStrokeVertexCount(self.handle, coat_index) };
        let triangles = unsafe { ffi::InProgressStrokeTriangleCount(self.handle, coat_index) };

        Mesh {
            positions: read_positions(vertices, |out, capacity| unsafe {
                ffi::InProgressStrokeCopyVertexPositions(self.handle, coat_index, out, capacity)
            }),
            indices: read_indices(triangles, |out, capacity| unsafe {
                ffi::InProgressStrokeCopyTriangleIndices(self.handle, coat_index, out, capacity)
            }),
        }
    }

    /// Bounding box of one coat, or `None` when it has no geometry yet.
    pub fn coat_bounds(&self, coat_index: u32) -> Option<Rect> {
        let mut out = ffi::Rect {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 0.0,
            y_max: 0.0,
        };
        let found = unsafe { ffi::InProgressStrokeCoatBounds(self.handle, coat_index, &mut out) };
        found.then(|| Rect::from_ffi(out))
    }

    /// Copies the current state into a finished [`Stroke`], leaving this
    /// builder alone so drawing can carry on.
    pub fn to_stroke(&self) -> Stroke {
        Stroke::from_ffi(unsafe { ffi::InProgressStrokeCopyToStroke(self.handle) })
    }
}

impl Default for InProgressStroke {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for InProgressStroke {
    fn drop(&mut self) {
        unsafe { ffi::InProgressStrokeDestroy(self.handle) };
    }
}

impl core::fmt::Debug for InProgressStroke {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InProgressStroke")
            .field("coat_count", &self.coat_count())
            .field("input_count", &self.input_count())
            .field("inputs_are_finished", &self.inputs_are_finished())
            .finish()
    }
}

/// A finished stroke: the input, the brush, and the geometry generated from
/// the two.
///
/// Geometry is organised as one render group per brush coat, each holding one
/// or more meshes. Ink splits a coat into several meshes when it has more
/// vertices than a single 16-bit index buffer can address, so a renderer should
/// loop over [`mesh_count`](Self::mesh_count) rather than assume there is one.
pub struct Stroke {
    handle: ffi::StrokeHandle,
}

impl Stroke {
    /// Generates a stroke from a brush and a complete run of input.
    pub fn new(brush: &Brush, inputs: &StrokeInputBatch) -> Self {
        Self::from_ffi(unsafe { ffi::StrokeCreate(brush.as_ffi(), inputs.as_ffi()) })
    }

    /// An empty stroke carrying just a brush.
    pub fn from_brush(brush: &Brush) -> Self {
        let empty = ffi::StrokeInputBatchHandle {
            handle: core::ptr::null_mut(),
        };
        Self::from_ffi(unsafe { ffi::StrokeCreate(brush.as_ffi(), empty) })
    }

    pub(crate) fn from_ffi(handle: ffi::StrokeHandle) -> Self {
        assert!(!handle.handle.is_null(), "ink: failed to allocate stroke");
        Self { handle }
    }

    /// Total time covered by the stroke's input.
    pub fn input_duration(&self) -> Duration {
        let seconds = unsafe { ffi::StrokeInputDurationSeconds(self.handle) };
        Duration::try_from_secs_f32(seconds).unwrap_or(Duration::ZERO)
    }

    /// A copy of the brush the stroke was drawn with.
    pub fn brush(&self) -> Brush {
        Brush::from_ffi(unsafe { ffi::StrokeGetBrush(self.handle) })
    }

    /// A copy of the input the stroke was generated from.
    pub fn inputs(&self) -> StrokeInputBatch {
        StrokeInputBatch::from_ffi(unsafe { ffi::StrokeGetInputs(self.handle) })
    }

    /// Number of render groups, one per brush coat.
    pub fn render_group_count(&self) -> u32 {
        unsafe { ffi::StrokeRenderGroupCount(self.handle) }
    }

    /// Number of meshes in one render group.
    pub fn mesh_count(&self, group_index: u32) -> u32 {
        unsafe { ffi::StrokeRenderGroupMeshCount(self.handle, group_index) }
    }

    /// One mesh. Out-of-range indices give an empty mesh.
    pub fn mesh(&self, group_index: u32, mesh_index: u32) -> Mesh {
        let vertices = unsafe { ffi::StrokeMeshVertexCount(self.handle, group_index, mesh_index) };
        let triangles =
            unsafe { ffi::StrokeMeshTriangleCount(self.handle, group_index, mesh_index) };

        Mesh {
            positions: read_positions(vertices, |out, capacity| unsafe {
                ffi::StrokeCopyMeshVertexPositions(
                    self.handle,
                    group_index,
                    mesh_index,
                    out,
                    capacity,
                )
            }),
            indices: read_indices(triangles, |out, capacity| unsafe {
                ffi::StrokeCopyMeshTriangleIndices(
                    self.handle,
                    group_index,
                    mesh_index,
                    out,
                    capacity,
                )
            }),
        }
    }

    /// Every mesh in the stroke, in the order they should be drawn.
    pub fn meshes(&self) -> Vec<Mesh> {
        let mut meshes = Vec::new();
        for group in 0..self.render_group_count() {
            for mesh in 0..self.mesh_count(group) {
                meshes.push(self.mesh(group, mesh));
            }
        }
        meshes
    }

    /// Bounding box of the whole stroke, or `None` when it has no geometry.
    pub fn bounds(&self) -> Option<Rect> {
        let mut out = ffi::Rect {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 0.0,
            y_max: 0.0,
        };
        let found = unsafe { ffi::StrokeBounds(self.handle, &mut out) };
        found.then(|| Rect::from_ffi(out))
    }
}

impl Clone for Stroke {
    fn clone(&self) -> Self {
        Self::from_ffi(unsafe { ffi::StrokeClone(self.handle) })
    }
}

impl Drop for Stroke {
    fn drop(&mut self) {
        unsafe { ffi::StrokeDestroy(self.handle) };
    }
}

impl core::fmt::Debug for Stroke {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Stroke")
            .field("render_group_count", &self.render_group_count())
            .field("input_duration", &self.input_duration())
            .finish()
    }
}
