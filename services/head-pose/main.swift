// One-shot local camera pose. No images, audio, files or network are produced.
import Foundation
import AVFoundation
import Vision
import CoreMedia
import CoreVideo

final class HeadPoseCapture: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    private let session = AVCaptureSession()
    private let lock = NSLock()
    private var finished = false
    private var lastFrame = 0.0

    func finish(_ pose: [String: Any]?, _ error: String? = nil) {
        lock.lock()
        if finished { lock.unlock(); return }
        finished = true
        lock.unlock()
        DispatchQueue.global().async {
            if self.session.isRunning { self.session.stopRunning() }
            if let pose = pose, let data = try? JSONSerialization.data(withJSONObject: pose) {
                FileHandle.standardOutput.write(data)
                FileHandle.standardOutput.write(Data([10]))
                exit(0)
            }
            FileHandle.standardError.write(Data((error ?? "Head pose unavailable").utf8))
            exit(1)
        }
    }

    func start() {
        DispatchQueue.global().async {
            do {
                guard let camera = AVCaptureDevice.default(for: .video) else {
                    self.finish(nil, "No camera found"); return
                }
                let input = try AVCaptureDeviceInput(device: camera)
                let output = AVCaptureVideoDataOutput()
                output.alwaysDiscardsLateVideoFrames = true
                output.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA]
                output.setSampleBufferDelegate(self, queue: DispatchQueue(label: "studio.kilsat.head-pose.frames"))
                self.session.beginConfiguration()
                self.session.sessionPreset = .vga640x480
                guard self.session.canAddInput(input), self.session.canAddOutput(output) else {
                    self.session.commitConfiguration()
                    self.finish(nil, "Camera configuration unavailable"); return
                }
                self.session.addInput(input)
                self.session.addOutput(output)
                self.session.commitConfiguration()
                self.session.startRunning()
                DispatchQueue.global().asyncAfter(deadline: .now() + 5) {
                    self.finish(nil, "No measurable face tilt found within five seconds")
                }
            } catch { self.finish(nil, "Camera error: \(error.localizedDescription)") }
        }
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
                       from connection: AVCaptureConnection) {
        let now = ProcessInfo.processInfo.systemUptime
        if now - lastFrame < 0.125 { return }
        lastFrame = now
        guard let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        do {
            let request = VNDetectFaceLandmarksRequest()
            try VNImageRequestHandler(cvPixelBuffer: pixels, orientation: .up).perform([request])
            guard let face = request.results?.filter({ $0.confidence >= 0.5 }).max(by: {
                $0.boundingBox.width * $0.boundingBox.height < $1.boundingBox.width * $1.boundingBox.height
            }), let roll = face.roll else { return }
            var pose: [String: Any] = ["roll": roll.doubleValue]
            if let yaw = face.yaw { pose["yaw"] = yaw.doubleValue }
            if let pitch = face.pitch { pose["pitch"] = pitch.doubleValue }
            finish(pose)
        } catch { finish(nil, "Face analysis failed: \(error.localizedDescription)") }
    }
}

let capture = HeadPoseCapture()
DispatchQueue.global().asyncAfter(deadline: .now() + 30) {
    capture.finish(nil, "Camera permission/capture timed out")
}
switch AVCaptureDevice.authorizationStatus(for: .video) {
case .authorized: capture.start()
case .notDetermined:
    AVCaptureDevice.requestAccess(for: .video) { allowed in
        if allowed { capture.start() }
        else { capture.finish(nil, "Camera access denied; check macOS Privacy & Security") }
    }
case .denied, .restricted:
    capture.finish(nil, "Camera access denied or restricted; check macOS Privacy & Security")
@unknown default: capture.finish(nil, "Unknown camera permission status")
}
RunLoop.main.run()
