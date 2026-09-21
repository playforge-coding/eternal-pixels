// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Draws a stroke the way an app would, frame by frame, and prints what Ink
//! produced along the way, ending with an ASCII picture of the geometry.
//!
//! Run with `bazel run //crates/ink-rs:stroke_example`, or with Cargo after
//! building an Ink prefix as the crate README describes.

use std::time::Duration;

use ink_rs::{
    Brush, Color, InProgressStroke, Mesh, Point, StockBrush, StrokeInput, StrokeInputBatch,
    ToolType,
};

/// A loop of pointer samples, one per 16ms frame, with pressure rising and
/// falling along the way.
fn samples() -> Vec<StrokeInput> {
    let count = 48;
    (0..count)
        .map(|i| {
            let t = i as f32 / (count - 1) as f32;
            let angle = t * std::f32::consts::TAU;
            let position = Point::new(
                60.0 + angle.cos() * 40.0,
                40.0 + angle.sin() * 25.0 + (angle * 3.0).sin() * 4.0,
            );
            StrokeInput::new(
                ToolType::Stylus,
                position,
                Duration::from_millis(i as u64 * 16),
            )
            .with_pressure(0.3 + 0.6 * (t * std::f32::consts::PI).sin())
        })
        .collect()
}

fn main() -> ink_rs::Result<()> {
    let family = StockBrush::PressurePen.family();
    let brush = Brush::new(&family, Color::from_packed_rgba(0x202020ff), 6.0, 0.1)?;
    println!(
        "brush: {} coat(s), size {}, epsilon {}",
        brush.coat_count(),
        brush.size(),
        brush.epsilon()
    );

    let samples = samples();
    let mut stroke = InProgressStroke::new();
    stroke.start(&brush);

    // Feed the samples in chunks, updating the shape after each, the way a
    // frame loop would feed the events that arrived since the last frame.
    for (frame, chunk) in samples.chunks(8).enumerate() {
        let batch = StrokeInputBatch::new(chunk)?;
        stroke.enqueue_inputs(&batch, None)?;
        let elapsed = chunk.last().map_or(Duration::ZERO, |s| s.elapsed_time);
        stroke.update_shape(elapsed)?;
        println!(
            "frame {frame}: {} inputs, {} triangles",
            stroke.input_count(),
            stroke.coat_mesh(0).triangle_count()
        );
    }

    stroke.finish_inputs();
    stroke.update_shape(Duration::from_millis(16 * 50))?;
    let finished = stroke.to_stroke();
    let meshes = finished.meshes();
    let bounds = finished.bounds().expect("the stroke has geometry");
    println!(
        "finished: {} mesh(es), {} triangles, bounds {}x{} around ({:.1}, {:.1})",
        meshes.len(),
        meshes.iter().map(Mesh::triangle_count).sum::<usize>(),
        bounds.width().round(),
        bounds.height().round(),
        bounds.center().x,
        bounds.center().y
    );
    println!();
    print_ascii(&meshes, bounds.center(), bounds.width(), bounds.height());
    Ok(())
}

/// Rasterises the triangles onto a character grid covering the bounds.
fn print_ascii(meshes: &[Mesh], center: Point, width: f32, height: f32) {
    const COLUMNS: usize = 72;
    const ROWS: usize = 24;
    let left = center.x - width / 2.0;
    let top = center.y - height / 2.0;
    let cell_w = width / COLUMNS as f32;
    let cell_h = height / ROWS as f32;
    for row in 0..ROWS {
        let mut line = String::with_capacity(COLUMNS);
        for column in 0..COLUMNS {
            let point = Point::new(
                left + (column as f32 + 0.5) * cell_w,
                top + (row as f32 + 0.5) * cell_h,
            );
            let covered = meshes.iter().any(|mesh| {
                (0..mesh.triangle_count())
                    .filter_map(|i| mesh.triangle(i))
                    .any(|triangle| contains(triangle, point))
            });
            line.push(if covered { '#' } else { '.' });
        }
        println!("{line}");
    }
}

fn contains([a, b, c]: [Point; 3], p: Point) -> bool {
    let sign = |p1: Point, p2: Point, p3: Point| {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    };
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}
