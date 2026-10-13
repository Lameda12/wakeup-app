# Models

| File | Source | Input | Outputs |
|---|---|---|---|
| `face_detection_short_range.onnx` | MediaPipe BlazeFace short range | `input` [1,128,128,3] RGB in [-1,1] | `regressors` [1,896,16], `classificators` [1,896,1] logits |
| `face_landmark.onnx` | MediaPipe Face Mesh | `input_1` [1,192,192,3] RGB in [0,1] | `conv2d_21` [1,1,1,1404] = 468 x (x,y,z) in 192px space, `conv2d_31` face presence logit |

Converted from `https://storage.googleapis.com/mediapipe-assets/<name>.tflite` with `tf2onnx` (opset 17) by `scripts/convert-models.sh`. Checked against the TFLite originals on the same input: max absolute output difference ~1e-4 for both.

License: MediaPipe and its models are published under Apache 2.0 `[Unverified: confirm on the model cards before shipping, and include the notice in the app's credits]`.
