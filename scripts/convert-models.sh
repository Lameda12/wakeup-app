#!/usr/bin/env bash
# Rebuilds models/*.onnx from MediaPipe's published TFLite files.
# Needs Python 3.10+. Usage: scripts/convert-models.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

python3 -m venv "$work/venv"
"$work/venv/bin/pip" install -q tf2onnx tensorflow-cpu onnxruntime

for model in face_detection_short_range face_landmark; do
  curl -fsSL -o "$work/$model.tflite" "https://storage.googleapis.com/mediapipe-assets/$model.tflite"
  "$work/venv/bin/python" -m tf2onnx.convert --tflite "$work/$model.tflite" \
    --output "$root/models/$model.onnx" --opset 17
done
echo "wrote $root/models/*.onnx"
