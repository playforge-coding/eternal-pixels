// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#ifndef ETERNAL_PIXELS_CRATES_INK_RS_CC_INK_FFI_H_
#define ETERNAL_PIXELS_CRATES_INK_RS_CC_INK_FFI_H_

// A Crubit-friendly facade over Google Ink.
//
// Ink's own API is built on `absl::StatusOr`, `absl::Span`, `std::vector` and
// templates. Crubit binds some of those, but not `absl::StatusOr` or
// `absl::Span`, which between them appear in most of Ink's public signatures.
// Running the generator straight at Ink's headers therefore drops nearly every
// interesting function.
//
// So this header deliberately includes nothing but <cstdint>. No Ink type and
// no standard library type appears in any signature below, which keeps the
// surface Crubit has to understand down to plain structs, enums and pointers.
// ink_ffi.cc is where the real Ink headers get included.
//
// Conventions:
//   - Owned Ink objects are handed out as one-word handle structs. Every
//     `*Create`/`*Clone` has a matching `*Destroy`; passing a handle to
//     `*Destroy` twice, or using it afterwards, is undefined.
//   - A null handle (`handle == nullptr`) is the empty/invalid handle. Creation
//     functions that cannot fail still return it when allocation fails.
//   - Fallible calls return `StatusCode`. On anything other than
//     `StatusCode::kOk`, `LastErrorMessage` describes what went wrong.
//   - Sentinel floats follow Ink: -1 means "absent" for pressure and the three
//     stylus angles, 0 means "absent" for stroke unit length.

#include <cstdint>

namespace ink_ffi {

// C linkage throughout. Type definitions are unaffected by this; what it
// changes is that the functions below get unmangled symbol names, so they can
// be declared from Rust without relying on C++ name mangling. That matters for
// the Cargo build, which does not run Crubit and declares them by hand.
extern "C" {

// Mirrors the subset of `absl::StatusCode` Ink actually returns. Values are the
// canonical status codes, so they match `absl::StatusCode` one for one.
enum class StatusCode : int32_t {
  kOk = 0,
  kUnknown = 2,
  kInvalidArgument = 3,
  kNotFound = 5,
  kFailedPrecondition = 9,
  kOutOfRange = 11,
  kUnimplemented = 12,
  kInternal = 13,
};

// Writes the message from the most recent failing call on this thread into
// `buffer` as NUL-terminated text, truncating if it does not fit. Returns the
// number of bytes written, excluding the terminator, or 0 if there is no
// message. Passing a null `buffer` or a `capacity` below 1 writes nothing.
int32_t LastErrorMessage(char* buffer, int32_t capacity);

//
// Value types
//

struct Point {
  float x;
  float y;
};

struct Vec2 {
  float x;
  float y;
};

struct Rect {
  float x_min;
  float y_min;
  float x_max;
  float y_max;
};

// Non-premultiplied unless the accompanying `ColorFormat` says otherwise.
struct Rgba {
  float r;
  float g;
  float b;
  float a;
};

enum class ColorSpace : int32_t {
  kSrgb = 0,
  kDisplayP3 = 1,
};

enum class ColorFormat : int32_t {
  kLinear = 0,
  kGammaEncoded = 1,
  kPremultipliedAlpha = 2,
};

enum class ToolType : int32_t {
  kUnknown = 0,
  kMouse = 1,
  kTouch = 2,
  kStylus = 3,
};

// One sampled input event. Mirrors `ink::StrokeInput` with the wrapper types
// flattened to floats: durations in seconds, distances in centimetres, angles
// in radians.
struct StrokeInput {
  ToolType tool_type;
  Point position;
  float elapsed_time_seconds;
  float stroke_unit_length_cm;  // 0 => absent
  float pressure;               // -1 => absent
  float tilt_radians;           // -1 => absent
  float orientation_radians;    // -1 => absent
  float barrel_twist_radians;   // -1 => absent
};

//
// Handles
//
// Each wraps an owning pointer to the corresponding Ink type. They are distinct
// struct types rather than a shared alias so that C++ still type-checks calls,
// and plain one-field structs rather than forward-declared classes so that
// Crubit has a complete type to bind.
//

struct BrushFamilyHandle {
  void* handle;
};

struct BrushHandle {
  void* handle;
};

struct StrokeInputBatchHandle {
  void* handle;
};

struct InProgressStrokeHandle {
  void* handle;
};

struct StrokeHandle {
  void* handle;
};

//
// Stock brush families
//
// `version` selects a versioned stock brush; 0 means "whatever is latest".
// Ink's stock brushes evolve, so pinning a version keeps previously drawn
// strokes looking the way they did when they were drawn.
//

BrushFamilyHandle StockBrushMarker(int32_t version);
BrushFamilyHandle StockBrushPressurePen(int32_t version);
BrushFamilyHandle StockBrushHighlighter(int32_t version);
BrushFamilyHandle StockBrushDashedLine(int32_t version);

BrushFamilyHandle BrushFamilyClone(BrushFamilyHandle family);
void BrushFamilyDestroy(BrushFamilyHandle family);

//
// Brush
//

// `size` and `epsilon` are in stroke units; both must be finite and positive,
// and `epsilon` must not exceed `size`.
StatusCode BrushCreate(BrushFamilyHandle family, Rgba color,
                       ColorSpace color_space, ColorFormat color_format,
                       float size, float epsilon, BrushHandle* out);

BrushHandle BrushClone(BrushHandle brush);
void BrushDestroy(BrushHandle brush);

float BrushSize(BrushHandle brush);
float BrushEpsilon(BrushHandle brush);
Rgba BrushColor(BrushHandle brush, ColorFormat color_format);
ColorSpace BrushColorSpace(BrushHandle brush);
uint32_t BrushCoatCount(BrushHandle brush);

// Returns a clone the caller owns, so that the brush keeps sole ownership of
// its own family.
BrushFamilyHandle BrushGetFamily(BrushHandle brush);

StatusCode BrushSetSize(BrushHandle brush, float size);
StatusCode BrushSetEpsilon(BrushHandle brush, float epsilon);
void BrushSetColor(BrushHandle brush, Rgba color, ColorSpace color_space,
                   ColorFormat color_format);
void BrushSetFamily(BrushHandle brush, BrushFamilyHandle family);

//
// Stroke input
//

// Inputs must be in non-decreasing time order and agree on tool type and on
// which optional fields are present; otherwise this fails with
// `kInvalidArgument`.
StatusCode StrokeInputBatchCreate(const StrokeInput* inputs, int32_t count,
                                  uint32_t noise_seed,
                                  float base_paint_animation_phase,
                                  StrokeInputBatchHandle* out);

StrokeInputBatchHandle StrokeInputBatchClone(StrokeInputBatchHandle batch);
void StrokeInputBatchDestroy(StrokeInputBatchHandle batch);

int32_t StrokeInputBatchSize(StrokeInputBatchHandle batch);

// Reading out of range yields a zeroed `StrokeInput`, so callers must check
// `StrokeInputBatchSize` first.
StrokeInput StrokeInputBatchGet(StrokeInputBatchHandle batch, int32_t index);

StatusCode StrokeInputBatchAppend(StrokeInputBatchHandle batch,
                                  StrokeInput input);
StatusCode StrokeInputBatchAppendBatch(StrokeInputBatchHandle batch,
                                       StrokeInputBatchHandle inputs);
void StrokeInputBatchClear(StrokeInputBatchHandle batch);

float StrokeInputBatchDurationSeconds(StrokeInputBatchHandle batch);
ToolType StrokeInputBatchToolType(StrokeInputBatchHandle batch);
uint32_t StrokeInputBatchNoiseSeed(StrokeInputBatchHandle batch);

//
// In-progress stroke
//
// The incremental builder: `Start`, then `EnqueueInputs`/`UpdateShape` as
// events arrive, then `FinishInputs` and `CopyToStroke`. The mesh readback
// functions exist so a renderer can pull geometry out each frame without Ink
// having to know about the renderer.
//

InProgressStrokeHandle InProgressStrokeCreate();
void InProgressStrokeDestroy(InProgressStrokeHandle stroke);

void InProgressStrokeClear(InProgressStrokeHandle stroke);
void InProgressStrokeStart(InProgressStrokeHandle stroke, BrushHandle brush,
                           uint32_t noise_seed, float base_animation_phase);

// `predicted` may be the null handle when there is no prediction.
StatusCode InProgressStrokeEnqueueInputs(InProgressStrokeHandle stroke,
                                         StrokeInputBatchHandle real_inputs,
                                         StrokeInputBatchHandle predicted_inputs);

void InProgressStrokeFinishInputs(InProgressStrokeHandle stroke);

// `current_elapsed_seconds` must not go backwards across calls.
StatusCode InProgressStrokeUpdateShape(InProgressStrokeHandle stroke,
                                       float current_elapsed_seconds);

bool InProgressStrokeNeedsUpdate(InProgressStrokeHandle stroke);
bool InProgressStrokeInputsAreFinished(InProgressStrokeHandle stroke);
bool InProgressStrokeChangesWithTime(InProgressStrokeHandle stroke);
uint32_t InProgressStrokeBrushCoatCount(InProgressStrokeHandle stroke);
int32_t InProgressStrokeInputCount(InProgressStrokeHandle stroke);
int32_t InProgressStrokeRealInputCount(InProgressStrokeHandle stroke);

uint32_t InProgressStrokeVertexCount(InProgressStrokeHandle stroke,
                                     uint32_t coat_index);
uint32_t InProgressStrokeTriangleCount(InProgressStrokeHandle stroke,
                                       uint32_t coat_index);

// Copy geometry for one brush coat into caller-owned storage. Both return the
// number of elements written, which is `min(capacity, count)`; `out` holds
// `capacity` elements, and triangle indices are written three per triangle.
int32_t InProgressStrokeCopyVertexPositions(InProgressStrokeHandle stroke,
                                            uint32_t coat_index, Point* out,
                                            int32_t capacity);
int32_t InProgressStrokeCopyTriangleIndices(InProgressStrokeHandle stroke,
                                            uint32_t coat_index, uint32_t* out,
                                            int32_t capacity);

// Writes the coat's bounding box and returns true, or returns false and leaves
// `out` untouched when the coat is empty.
bool InProgressStrokeCoatBounds(InProgressStrokeHandle stroke,
                                uint32_t coat_index, Rect* out);

StrokeHandle InProgressStrokeCopyToStroke(InProgressStrokeHandle stroke);

//
// Stroke
//
// A finished stroke. Its geometry is a partitioned mesh: one render group per
// brush coat, each holding one or more meshes.
//

// `inputs` may be the null handle for a stroke with no input yet.
StrokeHandle StrokeCreate(BrushHandle brush, StrokeInputBatchHandle inputs);
StrokeHandle StrokeClone(StrokeHandle stroke);
void StrokeDestroy(StrokeHandle stroke);

float StrokeInputDurationSeconds(StrokeHandle stroke);

// Both return clones the caller owns.
BrushHandle StrokeGetBrush(StrokeHandle stroke);
StrokeInputBatchHandle StrokeGetInputs(StrokeHandle stroke);

uint32_t StrokeRenderGroupCount(StrokeHandle stroke);
uint32_t StrokeRenderGroupMeshCount(StrokeHandle stroke, uint32_t group_index);
uint32_t StrokeMeshVertexCount(StrokeHandle stroke, uint32_t group_index,
                               uint32_t mesh_index);
uint32_t StrokeMeshTriangleCount(StrokeHandle stroke, uint32_t group_index,
                                 uint32_t mesh_index);
int32_t StrokeCopyMeshVertexPositions(StrokeHandle stroke, uint32_t group_index,
                                      uint32_t mesh_index, Point* out,
                                      int32_t capacity);
int32_t StrokeCopyMeshTriangleIndices(StrokeHandle stroke, uint32_t group_index,
                                      uint32_t mesh_index, uint32_t* out,
                                      int32_t capacity);

bool StrokeBounds(StrokeHandle stroke, Rect* out);

}  // extern "C"
}  // namespace ink_ffi

#endif  // ETERNAL_PIXELS_CRATES_INK_RS_CC_INK_FFI_H_
