import CoreGraphics
import Foundation
import QuartzCore

/// Playback and scrubbing state for one opened document.
///
/// Rendering runs on one background queue with at most one frame in flight:
/// when the core is slower than the frame rate, playback shows the newest
/// requested frame instead of queueing work (bounded, like the desktop preview).
@MainActor
final class PlayerModel: ObservableObject {
    @Published private(set) var document: AnimationDocument?
    @Published private(set) var fileName = ""
    @Published var sceneIndex = 0 { didSet { if sceneIndex != oldValue { seek(to: 0) } } }
    @Published private(set) var frame: UInt64 = 0
    @Published private(set) var image: CGImage?
    @Published private(set) var isPlaying = false
    @Published var errorMessage: String?

    /// Longest side requested from the core, set from the view size and screen scale.
    var renderSide: UInt32 = 768 {
        didSet { if renderSide != oldValue { requestRender() } }
    }

    private let queue = DispatchQueue(label: "studio.kilsat.render", qos: .userInitiated)
    private var rendering = false
    private var pendingFrame: UInt64?
    private var displayLink: CADisplayLink?
    private var playStart: (time: CFTimeInterval, frame: UInt64)?

    var scene: AnimationDocument.Scene? {
        guard let document, document.scenes.indices.contains(sceneIndex) else { return nil }
        return document.scenes[sceneIndex]
    }
    var frameCount: UInt64 { scene?.frameCount ?? 0 }
    var timecode: String {
        guard let document, document.fps > 0 else { return "" }
        let seconds = Double(frame) / Double(document.fps)
        return String(format: "%d:%05.2f  ·  ruutu %llu / %llu",
                      Int(seconds) / 60, seconds.truncatingRemainder(dividingBy: 60),
                      frame + 1, frameCount)
    }

    func open(url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        do {
            let data = try Data(contentsOf: url)
            load(data: data, name: url.lastPathComponent)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func openExample(named name: String) {
        guard let url = Bundle.main.url(forResource: name, withExtension: nil) else {
            errorMessage = "Esimerkkiä \(name) ei löytynyt sovelluksesta."
            return
        }
        open(url: url)
    }

    private func load(data: Data, name: String) {
        pause()
        let side = renderSide
        queue.async { [weak self] in
            let result = Result { try AnimationDocument(data: data) }
            // Render the first frame here too, so the preview never flashes empty.
            let first = result.flatMap { document in
                Result { try document.render(scene: 0, frame: 0, maxSide: side) }
            }
            DispatchQueue.main.async {
                guard let self else { return }
                switch result {
                case .success(let document):
                    self.document = document
                    self.fileName = name
                    self.sceneIndex = 0
                    self.frame = 0
                    self.image = try? first.get()
                    if case .failure(let error) = first { self.errorMessage = error.localizedDescription }
                case .failure(let error):
                    self.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func seek(to frame: UInt64) {
        guard frameCount > 0 else { return }
        self.frame = min(frame, frameCount - 1)
        if isPlaying { playStart = (CACurrentMediaTime(), self.frame) }
        requestRender()
    }

    func step(_ delta: Int64) {
        pause()
        let target = Int64(frame) + delta
        seek(to: UInt64(max(0, min(target, Int64(frameCount) - 1))))
    }

    func togglePlay() {
        isPlaying ? pause() : play()
    }

    func play() {
        guard let document, document.fps > 0, frameCount > 1 else { return }
        if frame >= frameCount - 1 { frame = 0 }
        playStart = (CACurrentMediaTime(), frame)
        isPlaying = true
        let link = CADisplayLink(target: DisplayLinkTarget { [weak self] in self?.tick() },
                                 selector: #selector(DisplayLinkTarget.fire))
        link.preferredFrameRateRange = CAFrameRateRange(
            minimum: Float(min(document.fps, 30)), maximum: Float(max(document.fps, 30)),
            preferred: Float(document.fps))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    func pause() {
        displayLink?.invalidate()
        displayLink = nil
        isPlaying = false
    }

    /// Frame from elapsed wall time, so slow rendering drops frames, never time.
    private func tick() {
        guard let document, let start = playStart, frameCount > 0 else { return pause() }
        let elapsed = CACurrentMediaTime() - start.time
        let target = start.frame + UInt64(max(0, elapsed) * Double(document.fps))
        if target >= frameCount {
            frame = frameCount - 1
            requestRender()
            pause()
        } else if target != frame {
            frame = target
            requestRender()
        }
    }

    private func requestRender() {
        guard document != nil else { return }
        pendingFrame = frame
        drainRender()
    }

    private func drainRender() {
        guard !rendering, let wanted = pendingFrame, let document, let scene else { return }
        pendingFrame = nil
        rendering = true
        let side = renderSide
        queue.async { [weak self] in
            let result = Result { try document.render(scene: scene.index, frame: wanted, maxSide: side) }
            DispatchQueue.main.async {
                guard let self else { return }
                self.rendering = false
                // Ignore frames for a document or scene that was replaced meanwhile.
                if self.document === document, self.scene == scene {
                    switch result {
                    case .success(let image): self.image = image
                    case .failure(let error):
                        self.pause()
                        self.errorMessage = error.localizedDescription
                    }
                }
                self.drainRender()
            }
        }
    }
}

/// CADisplayLink retains its target; this breaks the cycle with the model.
private final class DisplayLinkTarget: NSObject {
    let action: () -> Void
    init(_ action: @escaping () -> Void) { self.action = action }
    @objc func fire() { action() }
}
