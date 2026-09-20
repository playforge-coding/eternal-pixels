// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! End-to-end checks over the Ink bindings.
//!
//! These drive real Ink through the C++ facade, so they cover the parts most
//! likely to be wrong: handle ownership, the sentinel/`Option` mapping, status
//! propagation and mesh readback.

use std::time::Duration;

use ink_rs::{
    Brush, Color, ErrorKind, InProgressStroke, Point, StockBrush, Stroke, StrokeInput,
    StrokeInputBatch, ToolType,
};

fn test_brush() -> Brush {
    let family = StockBrush::Marker.family();
    Brush::new(&family, Color::from_packed_rgba(0x336699ff), 5.0, 0.1)
        .expect("a 5.0/0.1 marker brush is valid")
}

/// A short straight drag, spaced far enough apart to produce real geometry.
fn drag_inputs() -> Vec<StrokeInput> {
    (0..8u64)
        .map(|i| {
            let t = i as f32;
            StrokeInput::new(
                ToolType::Stylus,
                Point::new(10.0 + t * 6.0, 10.0 + t * 2.0),
                Duration::from_millis(i * 16),
            )
            .with_pressure(0.5)
        })
        .collect()
}

#[test]
fn brush_round_trips_its_parameters() {
    let brush = test_brush();

    assert_eq!(brush.size(), 5.0);
    assert_eq!(brush.epsilon(), 0.1);
    assert!(
        brush.coat_count() >= 1,
        "a stock brush paints at least once"
    );

    let color = brush.color();
    // Colour goes through a linear round trip inside Ink, so compare loosely.
    assert!((color.r - 0.2).abs() < 0.05, "red was {}", color.r);
    assert!((color.a - 1.0).abs() < 0.01, "alpha was {}", color.a);
}

#[test]
fn brush_rejects_a_nonsensical_size() {
    let family = StockBrush::Marker.family();

    let err = Brush::new(&family, Color::BLACK, -1.0, 0.1).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidArgument);
    assert!(!err.message().is_empty(), "Ink should say what went wrong");

    // Epsilon larger than size is also rejected.
    assert!(Brush::new(&family, Color::BLACK, 1.0, 2.0).is_err());
}

#[test]
fn brush_setters_validate() {
    let mut brush = test_brush();

    brush.set_size(9.0).expect("9.0 is a valid size");
    assert_eq!(brush.size(), 9.0);

    assert!(brush.set_size(f32::NAN).is_err());
    assert_eq!(brush.size(), 9.0, "a rejected size must not be applied");
}

#[test]
fn input_batch_preserves_inputs() {
    let inputs = drag_inputs();
    let batch = StrokeInputBatch::new(&inputs).expect("a monotonic stylus drag is valid");

    assert_eq!(batch.len(), inputs.len());
    assert_eq!(batch.tool_type(), ToolType::Stylus);
    // Not an exact comparison: Ink stores times as f32 seconds, so a duration
    // does not survive the round trip bit for bit.
    let drift = batch
        .duration()
        .abs_diff(Duration::from_millis(16 * 7))
        .as_secs_f32();
    assert!(drift < 1e-4, "duration drifted by {drift}s");

    let first = batch.get(0).expect("index 0 is in range");
    assert_eq!(first.position, Point::new(10.0, 10.0));
    assert_eq!(first.tool_type, ToolType::Stylus);
    assert!(first.pressure.is_some(), "pressure was supplied");
    assert!(first.tilt.is_none(), "tilt was not supplied");

    assert!(batch.get(inputs.len()).is_none(), "past the end reads None");
    assert_eq!(batch.iter().count(), inputs.len());
}

#[test]
fn input_batch_rejects_time_going_backwards() {
    let out_of_order = vec![
        StrokeInput::new(
            ToolType::Mouse,
            Point::new(0.0, 0.0),
            Duration::from_millis(50),
        ),
        StrokeInput::new(
            ToolType::Mouse,
            Point::new(1.0, 1.0),
            Duration::from_millis(10),
        ),
    ];

    let err = StrokeInputBatch::new(&out_of_order).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidArgument);
}

#[test]
fn input_batch_rejects_inconsistent_optional_fields() {
    let mixed = vec![
        StrokeInput::new(ToolType::Stylus, Point::new(0.0, 0.0), Duration::ZERO).with_pressure(0.5),
        StrokeInput::new(
            ToolType::Stylus,
            Point::new(5.0, 5.0),
            Duration::from_millis(16),
        ),
    ];

    assert!(
        StrokeInputBatch::new(&mixed).is_err(),
        "some inputs with pressure and some without should be rejected"
    );
}

#[test]
fn empty_batch_is_empty() {
    let batch = StrokeInputBatch::empty();
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);
    assert_eq!(batch.duration(), Duration::ZERO);
}

#[test]
fn in_progress_stroke_builds_geometry() {
    let brush = test_brush();
    let inputs = StrokeInputBatch::new(&drag_inputs()).unwrap();

    let mut stroke = InProgressStroke::new();
    stroke.start(&brush);
    assert_eq!(stroke.coat_count(), brush.coat_count());

    stroke.enqueue_inputs(&inputs, None).unwrap();
    assert!(stroke.needs_update(), "queued input makes the shape stale");

    stroke.update_shape(Duration::from_millis(112)).unwrap();
    assert_eq!(stroke.input_count(), stroke.real_input_count());

    let mesh = stroke.coat_mesh(0);
    assert!(!mesh.is_empty(), "a real drag should produce triangles");
    assert_eq!(
        mesh.indices.len() % 3,
        0,
        "indices come three to a triangle"
    );
    assert!(
        mesh.indices
            .iter()
            .all(|i| (*i as usize) < mesh.positions.len()),
        "every index must address a vertex"
    );
    assert!(mesh.triangle(0).is_some());

    let bounds = stroke
        .coat_bounds(0)
        .expect("non-empty geometry has bounds");
    assert!(bounds.width() > 0.0 && bounds.height() > 0.0);

    // Out-of-range coats read as empty rather than blowing up.
    assert!(stroke.coat_mesh(99).is_empty());
    assert!(stroke.coat_bounds(99).is_none());
}

#[test]
fn in_progress_stroke_converts_to_a_finished_stroke() {
    let brush = test_brush();
    let inputs = StrokeInputBatch::new(&drag_inputs()).unwrap();

    let mut in_progress = InProgressStroke::new();
    in_progress.start(&brush);
    in_progress.enqueue_inputs(&inputs, None).unwrap();
    in_progress.finish_inputs();
    in_progress
        .update_shape(Duration::from_millis(112))
        .unwrap();

    assert!(in_progress.inputs_are_finished());

    let stroke = in_progress.to_stroke();
    assert!(stroke.render_group_count() >= 1);
    assert_eq!(stroke.input_duration(), inputs.duration());

    let meshes = stroke.meshes();
    assert!(!meshes.is_empty(), "a finished stroke carries its geometry");
    assert!(meshes.iter().any(|m| !m.is_empty()));
    assert!(stroke.bounds().is_some());

    // The builder is untouched, so drawing can continue.
    assert!(!in_progress.coat_mesh(0).is_empty());
}

#[test]
fn stroke_built_directly_matches_its_inputs() {
    let brush = test_brush();
    let inputs = StrokeInputBatch::new(&drag_inputs()).unwrap();

    let stroke = Stroke::new(&brush, &inputs);
    assert_eq!(stroke.inputs().len(), inputs.len());
    assert_eq!(stroke.brush().size(), brush.size());
    assert!(stroke.bounds().is_some());
}

#[test]
fn clones_own_their_own_data() {
    let mut brush = test_brush();
    let mut clone = brush.clone();

    clone.set_size(20.0).unwrap();
    assert_eq!(brush.size(), 5.0, "the clone must not alias the original");
    assert_eq!(clone.size(), 20.0);

    brush.set_size(7.0).unwrap();
    assert_eq!(clone.size(), 20.0);

    let batch = StrokeInputBatch::new(&drag_inputs()).unwrap();
    let mut batch_clone = batch.clone();
    batch_clone.clear();
    assert!(batch_clone.is_empty());
    assert!(
        !batch.is_empty(),
        "clearing a clone must not clear the source"
    );
}

#[test]
fn empty_stroke_has_no_geometry() {
    let brush = test_brush();
    let stroke = Stroke::from_brush(&brush);

    assert!(stroke.bounds().is_none());
    assert!(stroke.meshes().iter().all(|m| m.is_empty()));
    assert_eq!(stroke.input_duration(), Duration::ZERO);
}
