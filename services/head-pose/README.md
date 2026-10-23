# Local one-shot head tilt capture

The helper uses macOS AVFoundation and Vision after an explicit Capture tilt
button. It adds no microphone input, saves no frames/audio and uses no network.
The largest detected face supplies roll and optional yaw/pitch. Capture stops
after its first measurable pose or five seconds without one; permission/setup
has a 30-second helper deadline and the parent has cancellation/60-second timeout.

Only 2D roll is copied to a reviewable head draft. A separate Apply writes an
ordinary keyframe. Articulated custom head joints support tilt; procedural
starter heads support X/Y movement only. This is not continuous motion capture.
Physical Mac camera permission, pose quality and head/rig direction calibration
remain unverified by automated protocol and packaging checks.

The Mac workflow compiles and signs the helper without opening a camera, retains
Swift compiler/source hash observations and inventories native binary hashes and
linkage alongside the app. No downloaded model or additional runtime is bundled.

Primary APIs:
- https://developer.apple.com/documentation/vision/vnfaceobservation
- https://developer.apple.com/documentation/avfoundation/avcapturedevice/requestaccess(for:completionhandler:)
