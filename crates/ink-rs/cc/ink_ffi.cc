// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Relative to this file, so the same source works whether it is compiled from
// the workspace root by Bazel or from the crate root by Cargo.
#include "ink_ffi.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "absl/status/status.h"
#include "absl/status/statusor.h"
#include "absl/types/span.h"
#include "ink/brush/brush.h"
#include "ink/brush/brush_family.h"
#include "ink/brush/brush_paint.h"
#include "ink/brush/stock_brushes.h"
#include "ink/color/color.h"
#include "ink/color/color_space.h"
#include "ink/geometry/envelope.h"
#include "ink/geometry/mesh.h"
#include "ink/geometry/mutable_mesh.h"
#include "ink/geometry/partitioned_mesh.h"
#include "ink/geometry/point.h"
#include "ink/geometry/rect.h"
#include "ink/strokes/in_progress_stroke.h"
#include "ink/strokes/input/stroke_input.h"
#include "ink/strokes/input/stroke_input_batch.h"
#include "ink/strokes/stroke.h"
#include "ink/types/duration.h"
#include "ink/types/physical_distance.h"

namespace ink_ffi {
namespace {

//
// Error reporting
//

std::string& LastErrorStorage() {
  thread_local std::string message;
  return message;
}

void ClearLastError() { LastErrorStorage().clear(); }

StatusCode RecordError(const absl::Status& status) {
  if (status.ok()) {
    ClearLastError();
    return StatusCode::kOk;
  }
  LastErrorStorage().assign(status.message().data(), status.message().size());
  return static_cast<StatusCode>(static_cast<int32_t>(status.code()));
}

// For failures we detect ourselves rather than getting from Ink.
StatusCode RecordError(StatusCode code, const char* message) {
  LastErrorStorage() = message;
  return code;
}

//
// Handle plumbing
//
// Every handle is a `void*` to a heap-allocated Ink object that the Rust side
// owns. `Unwrap` returns null for the null handle, so each entry point can
// check one pointer and bail out with a harmless default.
//

template <typename T, typename Handle>
T* Unwrap(Handle handle) {
  return static_cast<T*>(handle.handle);
}

template <typename Handle, typename T>
Handle Wrap(T* pointer) {
  return Handle{static_cast<void*>(pointer)};
}

template <typename Handle, typename T>
Handle WrapOwned(T&& value) {
  using Owned = std::remove_cvref_t<T>;
  return Wrap<Handle>(new Owned(std::forward<T>(value)));
}

template <typename T, typename Handle>
void DestroyHandle(Handle handle) {
  delete Unwrap<T>(handle);
}

//
// Value conversions
//

ink::Point ToInk(Point p) { return ink::Point{p.x, p.y}; }

Point FromInk(ink::Point p) { return Point{p.x, p.y}; }

Rect FromInk(const ink::Rect& r) {
  return Rect{r.XMin(), r.YMin(), r.XMax(), r.YMax()};
}

ink::ColorSpace ToInk(ColorSpace space) {
  return space == ColorSpace::kDisplayP3 ? ink::ColorSpace::kDisplayP3
                                         : ink::ColorSpace::kSrgb;
}

ColorSpace FromInk(ink::ColorSpace space) {
  return space == ink::ColorSpace::kDisplayP3 ? ColorSpace::kDisplayP3
                                              : ColorSpace::kSrgb;
}

ink::Color::Format ToInk(ColorFormat format) {
  switch (format) {
    case ColorFormat::kLinear:
      return ink::Color::Format::kLinear;
    case ColorFormat::kPremultipliedAlpha:
      return ink::Color::Format::kPremultipliedAlpha;
    case ColorFormat::kGammaEncoded:
      break;
  }
  return ink::Color::Format::kGammaEncoded;
}

ink::Color ToInk(Rgba color, ColorSpace space, ColorFormat format) {
  return ink::Color::FromFloat(color.r, color.g, color.b, color.a,
                               ToInk(format), ToInk(space));
}

Rgba FromInk(const ink::Color& color, ColorFormat format) {
  ink::Color::RgbaFloat rgba = color.AsFloat(ToInk(format));
  return Rgba{rgba.r, rgba.g, rgba.b, rgba.a};
}

ink::StrokeInput::ToolType ToInk(ToolType tool_type) {
  switch (tool_type) {
    case ToolType::kMouse:
      return ink::StrokeInput::ToolType::kMouse;
    case ToolType::kTouch:
      return ink::StrokeInput::ToolType::kTouch;
    case ToolType::kStylus:
      return ink::StrokeInput::ToolType::kStylus;
    case ToolType::kUnknown:
      break;
  }
  return ink::StrokeInput::ToolType::kUnknown;
}

ToolType FromInk(ink::StrokeInput::ToolType tool_type) {
  switch (tool_type) {
    case ink::StrokeInput::ToolType::kMouse:
      return ToolType::kMouse;
    case ink::StrokeInput::ToolType::kTouch:
      return ToolType::kTouch;
    case ink::StrokeInput::ToolType::kStylus:
      return ToolType::kStylus;
    case ink::StrokeInput::ToolType::kUnknown:
      break;
  }
  return ToolType::kUnknown;
}

ink::StrokeInput ToInk(const StrokeInput& input) {
  // The sentinels line up by construction: Ink treats a zero unit length and
  // -1 pressure/angles as "absent", and so does this facade.
  return ink::StrokeInput{
      .tool_type = ToInk(input.tool_type),
      .position = ToInk(input.position),
      .elapsed_time = ink::Duration32::Seconds(input.elapsed_time_seconds),
      .stroke_unit_length =
          ink::PhysicalDistance::Centimeters(input.stroke_unit_length_cm),
      .pressure = input.pressure,
      .tilt = ink::Angle::Radians(input.tilt_radians),
      .orientation = ink::Angle::Radians(input.orientation_radians),
      .barrel_twist = ink::Angle::Radians(input.barrel_twist_radians),
  };
}

StrokeInput FromInk(const ink::StrokeInput& input) {
  return StrokeInput{
      .tool_type = FromInk(input.tool_type),
      .position = FromInk(input.position),
      .elapsed_time_seconds = input.elapsed_time.ToSeconds(),
      .stroke_unit_length_cm = input.stroke_unit_length.ToCentimeters(),
      .pressure = input.pressure,
      .tilt_radians = input.tilt.ValueInRadians(),
      .orientation_radians = input.orientation.ValueInRadians(),
      .barrel_twist_radians = input.barrel_twist.ValueInRadians(),
  };
}

//
// Geometry readback
//
// `MutableMesh` and `Mesh` expose the same shape of API but share no base
// class, so the copy helpers are templates over both.
//

template <typename MeshType>
int32_t CopyVertexPositions(const MeshType& mesh, Point* out,
                            int32_t capacity) {
  if (out == nullptr || capacity <= 0) return 0;
  const int32_t count =
      std::min<int32_t>(capacity, static_cast<int32_t>(mesh.VertexCount()));
  for (int32_t i = 0; i < count; ++i) {
    out[i] = FromInk(mesh.VertexPosition(static_cast<uint32_t>(i)));
  }
  return count;
}

template <typename MeshType>
int32_t CopyTriangleIndices(const MeshType& mesh, uint32_t* out,
                            int32_t capacity) {
  if (out == nullptr || capacity <= 0) return 0;
  const int32_t triangles = std::min<int32_t>(
      capacity / 3, static_cast<int32_t>(mesh.TriangleCount()));
  for (int32_t i = 0; i < triangles; ++i) {
    std::array<uint32_t, 3> indices =
        mesh.TriangleIndices(static_cast<uint32_t>(i));
    out[i * 3 + 0] = indices[0];
    out[i * 3 + 1] = indices[1];
    out[i * 3 + 2] = indices[2];
  }
  return triangles * 3;
}

// Returns null when the group/mesh pair does not exist, so callers can treat an
// out-of-range index as an empty mesh instead of tripping Ink's CHECKs.
const ink::Mesh* LookupMesh(const ink::PartitionedMesh& shape,
                            uint32_t group_index, uint32_t mesh_index) {
  if (group_index >= shape.RenderGroupCount()) return nullptr;
  absl::Span<const ink::Mesh> meshes = shape.RenderGroupMeshes(group_index);
  if (mesh_index >= meshes.size()) return nullptr;
  return &meshes[mesh_index];
}

// Same idea for the in-progress stroke's per-coat meshes.
const ink::MutableMesh* LookupCoatMesh(const ink::InProgressStroke& stroke,
                                       uint32_t coat_index) {
  if (coat_index >= stroke.BrushCoatCount()) return nullptr;
  return &stroke.GetMesh(coat_index);
}

// `version == 0` means "latest", which each stock brush spells as its own
// enum's kLatest. The enumerators are numbered from 1, so 0 is free to mean it.
template <typename VersionEnum>
VersionEnum StockVersion(int32_t version) {
  if (version <= 0) return VersionEnum::kLatest;
  return static_cast<VersionEnum>(version);
}

}  // namespace

//
// Error reporting
//

int32_t LastErrorMessage(char* buffer, int32_t capacity) {
  if (buffer == nullptr || capacity < 1) return 0;
  const std::string& message = LastErrorStorage();
  const int32_t count =
      std::min<int32_t>(capacity - 1, static_cast<int32_t>(message.size()));
  std::memcpy(buffer, message.data(), static_cast<size_t>(count));
  buffer[count] = '\0';
  return count;
}

//
// Stock brush families
//

BrushFamilyHandle StockBrushMarker(int32_t version) {
  return WrapOwned<BrushFamilyHandle>(ink::stock_brushes::Marker(
      StockVersion<ink::stock_brushes::MarkerVersion>(version)));
}

BrushFamilyHandle StockBrushPressurePen(int32_t version) {
  return WrapOwned<BrushFamilyHandle>(ink::stock_brushes::PressurePen(
      StockVersion<ink::stock_brushes::PressurePenVersion>(version)));
}

BrushFamilyHandle StockBrushHighlighter(int32_t version) {
  // Highlighter takes its self-overlap mode before the version, unlike the
  // other stock brushes. `kAny` is the upstream default: it lets the renderer
  // pick whichever handling of a stroke crossing itself it can do best.
  return WrapOwned<BrushFamilyHandle>(ink::stock_brushes::Highlighter(
      ink::BrushPaint::SelfOverlap::kAny,
      StockVersion<ink::stock_brushes::HighlighterVersion>(version)));
}

BrushFamilyHandle StockBrushDashedLine(int32_t version) {
  return WrapOwned<BrushFamilyHandle>(ink::stock_brushes::DashedLine(
      StockVersion<ink::stock_brushes::DashedLineVersion>(version)));
}

BrushFamilyHandle BrushFamilyClone(BrushFamilyHandle family) {
  const auto* inner = Unwrap<ink::BrushFamily>(family);
  if (inner == nullptr) return BrushFamilyHandle{nullptr};
  return WrapOwned<BrushFamilyHandle>(*inner);
}

void BrushFamilyDestroy(BrushFamilyHandle family) {
  DestroyHandle<ink::BrushFamily>(family);
}

//
// Brush
//

StatusCode BrushCreate(BrushFamilyHandle family, Rgba color,
                       ColorSpace color_space, ColorFormat color_format,
                       float size, float epsilon, BrushHandle* out) {
  if (out == nullptr) {
    return RecordError(StatusCode::kInvalidArgument, "out must not be null");
  }
  *out = BrushHandle{nullptr};

  const auto* inner_family = Unwrap<ink::BrushFamily>(family);
  if (inner_family == nullptr) {
    return RecordError(StatusCode::kInvalidArgument,
                       "brush family handle is null");
  }

  absl::StatusOr<ink::Brush> brush = ink::Brush::Create(
      *inner_family, ToInk(color, color_space, color_format), size, epsilon);
  if (!brush.ok()) return RecordError(brush.status());

  *out = WrapOwned<BrushHandle>(*std::move(brush));
  ClearLastError();
  return StatusCode::kOk;
}

BrushHandle BrushClone(BrushHandle brush) {
  const auto* inner = Unwrap<ink::Brush>(brush);
  if (inner == nullptr) return BrushHandle{nullptr};
  return WrapOwned<BrushHandle>(*inner);
}

void BrushDestroy(BrushHandle brush) { DestroyHandle<ink::Brush>(brush); }

float BrushSize(BrushHandle brush) {
  const auto* inner = Unwrap<ink::Brush>(brush);
  return inner == nullptr ? 0.0f : inner->GetSize();
}

float BrushEpsilon(BrushHandle brush) {
  const auto* inner = Unwrap<ink::Brush>(brush);
  return inner == nullptr ? 0.0f : inner->GetEpsilon();
}

Rgba BrushColor(BrushHandle brush, ColorFormat color_format) {
  const auto* inner = Unwrap<ink::Brush>(brush);
  if (inner == nullptr) return Rgba{0.0f, 0.0f, 0.0f, 0.0f};
  return FromInk(inner->GetColor(), color_format);
}

ColorSpace BrushColorSpace(BrushHandle brush) {
  const auto* inner = Unwrap<ink::Brush>(brush);
  if (inner == nullptr) return ColorSpace::kSrgb;
  return FromInk(inner->GetColor().GetColorSpace());
}

uint32_t BrushCoatCount(BrushHandle brush) {
  const auto* inner = Unwrap<ink::Brush>(brush);
  return inner == nullptr ? 0 : inner->CoatCount();
}

BrushFamilyHandle BrushGetFamily(BrushHandle brush) {
  const auto* inner = Unwrap<ink::Brush>(brush);
  if (inner == nullptr) return BrushFamilyHandle{nullptr};
  return WrapOwned<BrushFamilyHandle>(inner->GetFamily());
}

StatusCode BrushSetSize(BrushHandle brush, float size) {
  auto* inner = Unwrap<ink::Brush>(brush);
  if (inner == nullptr) {
    return RecordError(StatusCode::kInvalidArgument, "brush handle is null");
  }
  return RecordError(inner->SetSize(size));
}

StatusCode BrushSetEpsilon(BrushHandle brush, float epsilon) {
  auto* inner = Unwrap<ink::Brush>(brush);
  if (inner == nullptr) {
    return RecordError(StatusCode::kInvalidArgument, "brush handle is null");
  }
  return RecordError(inner->SetEpsilon(epsilon));
}

void BrushSetColor(BrushHandle brush, Rgba color, ColorSpace color_space,
                   ColorFormat color_format) {
  auto* inner = Unwrap<ink::Brush>(brush);
  if (inner == nullptr) return;
  inner->SetColor(ToInk(color, color_space, color_format));
}

void BrushSetFamily(BrushHandle brush, BrushFamilyHandle family) {
  auto* inner = Unwrap<ink::Brush>(brush);
  const auto* inner_family = Unwrap<ink::BrushFamily>(family);
  if (inner == nullptr || inner_family == nullptr) return;
  inner->SetFamily(*inner_family);
}

//
// Stroke input
//

StatusCode StrokeInputBatchCreate(const StrokeInput* inputs, int32_t count,
                                  uint32_t noise_seed,
                                  float base_paint_animation_phase,
                                  StrokeInputBatchHandle* out) {
  if (out == nullptr) {
    return RecordError(StatusCode::kInvalidArgument, "out must not be null");
  }
  *out = StrokeInputBatchHandle{nullptr};

  if (count < 0 || (count > 0 && inputs == nullptr)) {
    return RecordError(StatusCode::kInvalidArgument,
                       "inputs/count pair is not a valid slice");
  }

  std::vector<ink::StrokeInput> converted;
  converted.reserve(static_cast<size_t>(count));
  for (int32_t i = 0; i < count; ++i) converted.push_back(ToInk(inputs[i]));

  absl::StatusOr<ink::StrokeInputBatch> batch = ink::StrokeInputBatch::Create(
      absl::MakeConstSpan(converted), noise_seed, base_paint_animation_phase);
  if (!batch.ok()) return RecordError(batch.status());

  *out = WrapOwned<StrokeInputBatchHandle>(*std::move(batch));
  ClearLastError();
  return StatusCode::kOk;
}

StrokeInputBatchHandle StrokeInputBatchClone(StrokeInputBatchHandle batch) {
  const auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  if (inner == nullptr) return StrokeInputBatchHandle{nullptr};
  // Ink's copy constructor is copy-on-write; a deep copy keeps the clone from
  // sharing storage that the Rust side believes it owns outright.
  return WrapOwned<StrokeInputBatchHandle>(inner->MakeDeepCopy());
}

void StrokeInputBatchDestroy(StrokeInputBatchHandle batch) {
  DestroyHandle<ink::StrokeInputBatch>(batch);
}

int32_t StrokeInputBatchSize(StrokeInputBatchHandle batch) {
  const auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  return inner == nullptr ? 0 : inner->Size();
}

StrokeInput StrokeInputBatchGet(StrokeInputBatchHandle batch, int32_t index) {
  const auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  if (inner == nullptr || index < 0 || index >= inner->Size()) {
    return StrokeInput{};
  }
  return FromInk(inner->Get(index));
}

StatusCode StrokeInputBatchAppend(StrokeInputBatchHandle batch,
                                  StrokeInput input) {
  auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  if (inner == nullptr) {
    return RecordError(StatusCode::kInvalidArgument, "batch handle is null");
  }
  return RecordError(inner->Append(ToInk(input)));
}

StatusCode StrokeInputBatchAppendBatch(StrokeInputBatchHandle batch,
                                       StrokeInputBatchHandle inputs) {
  auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  const auto* appended = Unwrap<ink::StrokeInputBatch>(inputs);
  if (inner == nullptr) {
    return RecordError(StatusCode::kInvalidArgument, "batch handle is null");
  }
  if (appended == nullptr) {
    ClearLastError();
    return StatusCode::kOk;
  }
  return RecordError(inner->Append(*appended));
}

void StrokeInputBatchClear(StrokeInputBatchHandle batch) {
  auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  if (inner != nullptr) inner->Clear();
}

float StrokeInputBatchDurationSeconds(StrokeInputBatchHandle batch) {
  const auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  return inner == nullptr ? 0.0f : inner->GetDuration().ToSeconds();
}

ToolType StrokeInputBatchToolType(StrokeInputBatchHandle batch) {
  const auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  if (inner == nullptr) return ToolType::kUnknown;
  return FromInk(inner->GetToolType());
}

uint32_t StrokeInputBatchNoiseSeed(StrokeInputBatchHandle batch) {
  const auto* inner = Unwrap<ink::StrokeInputBatch>(batch);
  return inner == nullptr ? 0 : inner->GetNoiseSeed();
}

//
// In-progress stroke
//

InProgressStrokeHandle InProgressStrokeCreate() {
  return Wrap<InProgressStrokeHandle>(new ink::InProgressStroke());
}

void InProgressStrokeDestroy(InProgressStrokeHandle stroke) {
  DestroyHandle<ink::InProgressStroke>(stroke);
}

void InProgressStrokeClear(InProgressStrokeHandle stroke) {
  auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner != nullptr) inner->Clear();
}

void InProgressStrokeStart(InProgressStrokeHandle stroke, BrushHandle brush,
                           uint32_t noise_seed, float base_animation_phase) {
  auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  const auto* inner_brush = Unwrap<ink::Brush>(brush);
  if (inner == nullptr || inner_brush == nullptr) return;
  inner->Start(*inner_brush, noise_seed, base_animation_phase);
}

StatusCode InProgressStrokeEnqueueInputs(
    InProgressStrokeHandle stroke, StrokeInputBatchHandle real_inputs,
    StrokeInputBatchHandle predicted_inputs) {
  auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner == nullptr) {
    return RecordError(StatusCode::kInvalidArgument, "stroke handle is null");
  }

  // Ink takes both batches by reference, so a null handle becomes an empty
  // batch rather than an error: predicting nothing is a normal thing to do.
  const ink::StrokeInputBatch empty;
  const auto* real = Unwrap<ink::StrokeInputBatch>(real_inputs);
  const auto* predicted = Unwrap<ink::StrokeInputBatch>(predicted_inputs);
  return RecordError(inner->EnqueueInputs(real == nullptr ? empty : *real,
                                          predicted == nullptr ? empty
                                                               : *predicted));
}

void InProgressStrokeFinishInputs(InProgressStrokeHandle stroke) {
  auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner != nullptr) inner->FinishInputs();
}

StatusCode InProgressStrokeUpdateShape(InProgressStrokeHandle stroke,
                                       float current_elapsed_seconds) {
  auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner == nullptr) {
    return RecordError(StatusCode::kInvalidArgument, "stroke handle is null");
  }
  return RecordError(
      inner->UpdateShape(ink::Duration32::Seconds(current_elapsed_seconds)));
}

bool InProgressStrokeNeedsUpdate(InProgressStrokeHandle stroke) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  return inner != nullptr && inner->NeedsUpdate();
}

bool InProgressStrokeInputsAreFinished(InProgressStrokeHandle stroke) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  return inner != nullptr && inner->InputsAreFinished();
}

bool InProgressStrokeChangesWithTime(InProgressStrokeHandle stroke) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  return inner != nullptr && inner->ChangesWithTime();
}

uint32_t InProgressStrokeBrushCoatCount(InProgressStrokeHandle stroke) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  return inner == nullptr ? 0 : inner->BrushCoatCount();
}

int32_t InProgressStrokeInputCount(InProgressStrokeHandle stroke) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  return inner == nullptr ? 0 : inner->InputCount();
}

int32_t InProgressStrokeRealInputCount(InProgressStrokeHandle stroke) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  return inner == nullptr ? 0 : inner->RealInputCount();
}

uint32_t InProgressStrokeVertexCount(InProgressStrokeHandle stroke,
                                     uint32_t coat_index) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::MutableMesh* mesh = LookupCoatMesh(*inner, coat_index);
  return mesh == nullptr ? 0 : mesh->VertexCount();
}

uint32_t InProgressStrokeTriangleCount(InProgressStrokeHandle stroke,
                                       uint32_t coat_index) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::MutableMesh* mesh = LookupCoatMesh(*inner, coat_index);
  return mesh == nullptr ? 0 : mesh->TriangleCount();
}

int32_t InProgressStrokeCopyVertexPositions(InProgressStrokeHandle stroke,
                                            uint32_t coat_index, Point* out,
                                            int32_t capacity) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::MutableMesh* mesh = LookupCoatMesh(*inner, coat_index);
  return mesh == nullptr ? 0 : CopyVertexPositions(*mesh, out, capacity);
}

int32_t InProgressStrokeCopyTriangleIndices(InProgressStrokeHandle stroke,
                                            uint32_t coat_index, uint32_t* out,
                                            int32_t capacity) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::MutableMesh* mesh = LookupCoatMesh(*inner, coat_index);
  return mesh == nullptr ? 0 : CopyTriangleIndices(*mesh, out, capacity);
}

bool InProgressStrokeCoatBounds(InProgressStrokeHandle stroke,
                                uint32_t coat_index, Rect* out) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner == nullptr || out == nullptr) return false;
  if (coat_index >= inner->BrushCoatCount()) return false;

  const std::optional<ink::Rect>& bounds =
      inner->GetMeshBounds(coat_index).AsRect();
  if (!bounds.has_value()) return false;

  *out = FromInk(*bounds);
  return true;
}

StrokeHandle InProgressStrokeCopyToStroke(InProgressStrokeHandle stroke) {
  const auto* inner = Unwrap<ink::InProgressStroke>(stroke);
  if (inner == nullptr) return StrokeHandle{nullptr};
  return WrapOwned<StrokeHandle>(inner->CopyToStroke());
}

//
// Stroke
//

StrokeHandle StrokeCreate(BrushHandle brush, StrokeInputBatchHandle inputs) {
  const auto* inner_brush = Unwrap<ink::Brush>(brush);
  if (inner_brush == nullptr) return StrokeHandle{nullptr};

  const auto* inner_inputs = Unwrap<ink::StrokeInputBatch>(inputs);
  if (inner_inputs == nullptr) {
    return WrapOwned<StrokeHandle>(ink::Stroke(*inner_brush));
  }
  return WrapOwned<StrokeHandle>(ink::Stroke(*inner_brush, *inner_inputs));
}

StrokeHandle StrokeClone(StrokeHandle stroke) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr) return StrokeHandle{nullptr};
  return WrapOwned<StrokeHandle>(*inner);
}

void StrokeDestroy(StrokeHandle stroke) { DestroyHandle<ink::Stroke>(stroke); }

float StrokeInputDurationSeconds(StrokeHandle stroke) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  return inner == nullptr ? 0.0f : inner->GetInputDuration().ToSeconds();
}

BrushHandle StrokeGetBrush(StrokeHandle stroke) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr) return BrushHandle{nullptr};
  return WrapOwned<BrushHandle>(inner->GetBrush());
}

StrokeInputBatchHandle StrokeGetInputs(StrokeHandle stroke) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr) return StrokeInputBatchHandle{nullptr};
  return WrapOwned<StrokeInputBatchHandle>(inner->GetInputs().MakeDeepCopy());
}

uint32_t StrokeRenderGroupCount(StrokeHandle stroke) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  return inner == nullptr ? 0 : inner->GetShape().RenderGroupCount();
}

uint32_t StrokeRenderGroupMeshCount(StrokeHandle stroke, uint32_t group_index) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::PartitionedMesh& shape = inner->GetShape();
  if (group_index >= shape.RenderGroupCount()) return 0;
  return static_cast<uint32_t>(shape.RenderGroupMeshes(group_index).size());
}

uint32_t StrokeMeshVertexCount(StrokeHandle stroke, uint32_t group_index,
                               uint32_t mesh_index) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::Mesh* mesh =
      LookupMesh(inner->GetShape(), group_index, mesh_index);
  return mesh == nullptr ? 0 : mesh->VertexCount();
}

uint32_t StrokeMeshTriangleCount(StrokeHandle stroke, uint32_t group_index,
                                 uint32_t mesh_index) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::Mesh* mesh =
      LookupMesh(inner->GetShape(), group_index, mesh_index);
  return mesh == nullptr ? 0 : mesh->TriangleCount();
}

int32_t StrokeCopyMeshVertexPositions(StrokeHandle stroke, uint32_t group_index,
                                      uint32_t mesh_index, Point* out,
                                      int32_t capacity) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::Mesh* mesh =
      LookupMesh(inner->GetShape(), group_index, mesh_index);
  return mesh == nullptr ? 0 : CopyVertexPositions(*mesh, out, capacity);
}

int32_t StrokeCopyMeshTriangleIndices(StrokeHandle stroke, uint32_t group_index,
                                      uint32_t mesh_index, uint32_t* out,
                                      int32_t capacity) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr) return 0;
  const ink::Mesh* mesh =
      LookupMesh(inner->GetShape(), group_index, mesh_index);
  return mesh == nullptr ? 0 : CopyTriangleIndices(*mesh, out, capacity);
}

bool StrokeBounds(StrokeHandle stroke, Rect* out) {
  const auto* inner = Unwrap<ink::Stroke>(stroke);
  if (inner == nullptr || out == nullptr) return false;

  const ink::Envelope bounds = inner->GetShape().Bounds();
  const std::optional<ink::Rect>& rect = bounds.AsRect();
  if (!rect.has_value()) return false;

  *out = FromInk(*rect);
  return true;
}

}  // namespace ink_ffi
