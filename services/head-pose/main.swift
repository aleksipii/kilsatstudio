// Explicit local camera pose capture. No audio, saved images, files or network.
import Foundation
import Darwin
import AVFoundation
import Vision
import CoreMedia
import CoreVideo

struct Options {
    var stream = false
    var duration = 60.0
    var fps = 12
}
func usage(_ error: String? = nil) -> Never {
    let help = "Käyttö: head_pose [--stream [--duration SEKUNTIA(1..60)] [--fps FPS(1..20)]]\nIlman valitsimia: yksi pään kallistus. --help ei avaa kameraa.\n"
    if let error = error { FileHandle.standardError.write(Data((error + "\n" + help).utf8)); exit(2) }
    FileHandle.standardOutput.write(Data(help.utf8)); exit(0)
}
let arguments = Array(CommandLine.arguments.dropFirst())
if arguments == ["--help"] || arguments == ["-h"] { usage() }
var options = Options()
var argument = 0
var seen = Set<String>()
while argument < arguments.count {
    let flag = arguments[argument]
    guard !seen.contains(flag) else { usage("Valitsin annettiin useammin kuin kerran: \(flag)") }
    seen.insert(flag)
    switch flag {
    case "--stream": options.stream = true
    case "--duration":
        argument += 1
        guard argument < arguments.count, let value = Double(arguments[argument]), value.isFinite,
              value >= 1, value <= 60 else { usage("Keston on oltava 1–60 sekuntia") }
        options.duration = value
    case "--fps":
        argument += 1
        guard argument < arguments.count, let value = Int(arguments[argument]), value >= 1, value <= 20
        else { usage("FPS-arvon on oltava 1–20") }
        options.fps = value
    default: usage("Tuntematon valitsin: \(flag)")
    }
    argument += 1
}
if !options.stream && (seen.contains("--fps") || seen.contains("--duration")) { usage("Ajastusvalitsimet vaativat --stream-valitsimen") }

final class HeadPoseCapture: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    private let session = AVCaptureSession()
    private let lock = NSLock()
    private var finished = false
    private var started: Double?
    private var configured = false
    private var lastFace = 0.0
    private var lastFrame = 0.0
    private var sequence = 0
    private let options: Options
    init(_ options: Options) { self.options = options; super.init() }

    func finish(_ error: String? = nil, pose: [String: Any]? = nil) {
        lock.lock()
        if finished { lock.unlock(); return }
        finished = true
        lock.unlock()
        DispatchQueue.global().async {
            if self.session.isRunning { self.session.stopRunning() }
            if let error = error {
                FileHandle.standardError.write(Data((error + "\n").utf8)); exit(1)
            }
            if let pose = pose, let data = try? JSONSerialization.data(withJSONObject: pose) {
                FileHandle.standardOutput.write(data); FileHandle.standardOutput.write(Data([10]))
            }
            exit(0)
        }
    }
    func permissionDeadline() {
        lock.lock(); let incomplete = !configured && !finished; lock.unlock()
        if incomplete { finish("Kameran käyttöluvan tai käynnistyksen aikaraja ylittyi") }
    }
    private func checkDeadline() {
        lock.lock()
        let done = finished
        let start = started
        let last = lastFace
        lock.unlock()
        if done { return }
        if let start = start {
            let now = ProcessInfo.processInfo.systemUptime
            if options.stream && now - start >= options.duration { finish(); return }
            if now - last >= 5 { finish("Kasvoja ei voitu mitata viiteen sekuntiin"); return }
        }
        DispatchQueue.global().asyncAfter(deadline: .now() + 0.25) { self.checkDeadline() }
    }
    func start() {
        DispatchQueue.global().async {
            do {
                guard let camera = AVCaptureDevice.default(for: .video) else { self.finish("Kameraa ei löytynyt"); return }
                let input = try AVCaptureDeviceInput(device: camera)
                let output = AVCaptureVideoDataOutput()
                output.alwaysDiscardsLateVideoFrames = true
                output.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA]
                output.setSampleBufferDelegate(self, queue: DispatchQueue(label: "studio.kilsat.head-pose.frames"))
                self.session.beginConfiguration()
                self.session.sessionPreset = .vga640x480
                guard self.session.canAddInput(input), self.session.canAddOutput(output) else {
                    self.session.commitConfiguration(); self.finish("Kameran asetuksia ei voitu ottaa käyttöön"); return
                }
                self.session.addInput(input); self.session.addOutput(output); self.session.commitConfiguration()
                self.lock.lock(); let done = self.finished; self.lock.unlock()
                if done { return }
                // Establish a monotonic capture origin before starting callback delivery.
                let now = ProcessInfo.processInfo.systemUptime
                self.lock.lock(); self.started = now; self.lastFace = now; self.lock.unlock()
                self.session.startRunning()
                self.lock.lock(); self.configured = true; self.lock.unlock()
                self.checkDeadline()
            } catch { self.finish("Kameravirhe: \(error.localizedDescription)") }
        }
    }
    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
                       from connection: AVCaptureConnection) {
        let now = ProcessInfo.processInfo.systemUptime
        lock.lock(); let done = finished; lock.unlock()
        if done || now - lastFrame < 1.0 / Double(options.stream ? options.fps : 8) { return }
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
            let angles = [face.roll, face.yaw, face.pitch].compactMap { $0?.doubleValue }
            guard angles.allSatisfy({ $0.isFinite && abs($0) <= Double.pi }), abs(roll.doubleValue) <= Double.pi / 2 else { return }
            lock.lock()
            if finished { lock.unlock(); return }
            lastFace = now
            let elapsed = max(0, now - (started ?? now))
            let seq = sequence; sequence += 1
            lock.unlock()
            if !options.stream { finish(pose: pose); return }
            if elapsed >= options.duration || seq >= 1200 { finish(); return }
            pose["seq"] = seq; pose["time"] = elapsed
            let data = try JSONSerialization.data(withJSONObject: pose)
            FileHandle.standardOutput.write(data); FileHandle.standardOutput.write(Data([10]))
        } catch { finish("Kasvojen analysointi epäonnistui: \(error.localizedDescription)") }
    }
}

// A camera child must not outlive the desktop that requested it.
let parentPID = getppid()
func watchParent() {
    if getppid() != parentPID { _exit(1) }
    DispatchQueue.global().asyncAfter(deadline: .now() + 0.25) { watchParent() }
}
watchParent()
let capture = HeadPoseCapture(options)
DispatchQueue.global().asyncAfter(deadline: .now() + 30) { capture.permissionDeadline() }
// Last-resort bound even if configuration/callback delivery unexpectedly stalls.
DispatchQueue.global().asyncAfter(deadline: .now() + 92) { capture.finish("Kamera-apuohjelman aikaraja ylittyi") }
switch AVCaptureDevice.authorizationStatus(for: .video) {
case .authorized: capture.start()
case .notDetermined:
    AVCaptureDevice.requestAccess(for: .video) { allowed in
        if allowed { capture.start() } else { capture.finish("Kameran käyttö estettiin. Tarkista macOS:n Tietosuoja ja suojaus -asetukset") }
    }
case .denied, .restricted: capture.finish("Kameran käyttö on estetty tai rajoitettu. Tarkista macOS:n Tietosuoja ja suojaus -asetukset")
@unknown default: capture.finish("Tuntematon kameran käyttöluvan tila")
}
RunLoop.main.run()
