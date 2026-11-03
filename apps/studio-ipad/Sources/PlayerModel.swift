import AVFoundation
import CoreGraphics
import Foundation
import QuartzCore

/// Playback and scrubbing state for one opened document.
///
/// Rendering runs on one background queue with at most one frame in flight:
/// when the core is slower than the frame rate, playback shows the newest
/// requested frame instead of queueing work (bounded, like the desktop preview).
///
/// When the scene has audio, the audio player's sample position is the clock:
/// the shown frame is the core's `frame(atSample:)` of the sample being heard,
/// the same rule export uses. Without audio, elapsed wall time is the clock.
@MainActor
final class PlayerModel: ObservableObject {
    enum AudioStatus: Equatable {
        /// The scene has no dialogue or audio cues.
        case none
        case loading
        case ready
        /// Audio exists in the screenplay but cannot be played; the message says why.
        case unavailable(String)
    }

    @Published private(set) var document: AnimationDocument?
    @Published private(set) var fileName = ""
    @Published var sceneIndex = 0 {
        didSet { if sceneIndex != oldValue { pause(); seek(to: 0); loadAudio() } }
    }
    @Published private(set) var frame: UInt64 = 0
    @Published private(set) var image: CGImage?
    @Published private(set) var isPlaying = false
    @Published private(set) var audioStatus = AudioStatus.none
    @Published var errorMessage: String?
    /// Project files found in a folder the user opened, when there are several.
    @Published var folderChoices: [URL] = []

    /// Longest side requested from the core, set from the view size and screen scale.
    var renderSide: UInt32 = 768 {
        didSet { if renderSide != oldValue { requestRender() } }
    }

    private let queue = DispatchQueue(label: "studio.kilsat.render", qos: .userInitiated)
    private var rendering = false
    private var pendingFrame: UInt64?
    private var displayLink: CADisplayLink?
    private var playStart: (time: CFTimeInterval, frame: UInt64)?

    private let audioQueue = DispatchQueue(label: "studio.kilsat.audio", qos: .userInitiated)
    private var audioLoading = false
    private var audioWanted = false
    private var audioBuffer: AVAudioPCMBuffer?
    private let engine = AVAudioEngine()
    private let audioPlayer = AVAudioPlayerNode()
    /// Scene sample at which the playing audio segment starts, while audio is the clock.
    private var audioClockStart: UInt64?
    /// Security-scoped file or folder kept open while its document is open,
    /// because imported audio is read after the document opened.
    private var scopedURL: URL?
    /// Start playback once the scene's audio is settled (launch smoke tests).
    var playWhenReady = false
    /// Folder access held while the user chooses among `folderChoices`.
    private var pendingFolderScope: URL?
    private var interruptionObserver: NSObjectProtocol?

    init() {
        engine.attach(audioPlayer)
        if let format = AVAudioFormat(standardFormatWithSampleRate: 24_000, channels: 1) {
            engine.connect(audioPlayer, to: engine.mainMixerNode, format: format)
        }
        interruptionObserver = NotificationCenter.default.addObserver(
            forName: AVAudioSession.interruptionNotification, object: nil, queue: .main
        ) { [weak self] _ in
            Task { @MainActor in self?.pause() }
        }
    }

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

    /// Open one file. A file picked on its own may not grant access to the
    /// images and audio next to it; opening its folder does.
    func open(url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        load(url: url, scope: scoped ? url : nil)
    }

    /// Open a project folder: grants access to everything in it, then opens its
    /// only project file or lets the user choose among several.
    func open(folder: URL) {
        let scoped = folder.startAccessingSecurityScopedResource()
        let files = (try? FileManager.default.contentsOfDirectory(
            at: folder, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles])) ?? []
        let projects = files
            .filter { ["kilsat", "yaml", "yml"].contains($0.pathExtension.lowercased()) }
            .sorted { $0.lastPathComponent.localizedStandardCompare($1.lastPathComponent) == .orderedAscending }
        switch projects.count {
        case 0:
            if scoped { folder.stopAccessingSecurityScopedResource() }
            errorMessage = "Kansiossa ei ole .kilsat- tai .yaml-tiedostoa."
        case 1:
            load(url: projects[0], scope: scoped ? folder : nil)
        default:
            cancelFolderChoice()
            pendingFolderScope = scoped ? folder : nil
            folderChoices = projects
        }
    }

    /// Open one of `folderChoices` with the folder's access.
    func choose(_ url: URL) {
        let scope = pendingFolderScope
        pendingFolderScope = nil
        folderChoices = []
        load(url: url, scope: scope)
    }

    /// The user closed the folder's file list without choosing.
    func cancelFolderChoice() {
        folderChoices = []
        if let pendingFolderScope, pendingFolderScope != scopedURL {
            pendingFolderScope.stopAccessingSecurityScopedResource()
        }
        pendingFolderScope = nil
    }

    /// Bundled examples, then files copied into the app's own Files folder.
    /// `scene` selects a scene once it has opened (launch smoke tests).
    func openExample(named name: String, scene: Int? = nil) {
        let candidates = [
            Bundle.main.resourceURL?.appendingPathComponent(name),
            FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?
                .appendingPathComponent(name),
        ].compactMap { $0 }
        guard let url = candidates.first(where: { FileManager.default.fileExists(atPath: $0.path) }) else {
            errorMessage = "Esimerkkiä \(name) ei löytynyt sovelluksesta."
            return
        }
        load(url: url, scope: nil, scene: scene)
    }

    private func replaceScope(with url: URL?) {
        if let scopedURL, scopedURL != url { scopedURL.stopAccessingSecurityScopedResource() }
        scopedURL = url
    }

    private func load(url: URL, scope: URL?, scene: Int? = nil) {
        pause()
        let side = renderSide
        queue.async { [weak self] in
            let result = Result { try AnimationDocument(url: url) }
            // Render the first frame here too, so the preview never flashes empty.
            let first = result.flatMap { document in
                Result { try document.render(scene: 0, frame: 0, maxSide: side) }
            }
            DispatchQueue.main.async {
                guard let self else { return }
                switch result {
                case .success(let document):
                    self.replaceScope(with: scope)
                    self.document = document
                    self.fileName = url.lastPathComponent
                    self.sceneIndex = 0
                    self.frame = 0
                    self.image = try? first.get()
                    if case .failure(let error) = first { self.errorMessage = error.localizedDescription }
                    self.loadAudio()
                    if let scene, document.scenes.indices.contains(scene) { self.sceneIndex = scene }
                case .failure(let error):
                    if let scope, scope != self.scopedURL {
                        scope.stopAccessingSecurityScopedResource()
                    }
                    self.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func seek(to frame: UInt64) {
        guard frameCount > 0 else { return }
        self.frame = min(frame, frameCount - 1)
        if isPlaying {
            playStart = (CACurrentMediaTime(), self.frame)
            startAudio()
        }
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
        startAudio()
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
        stopAudio()
    }

    /// Start the prepared scene audio at the current frame's first sample.
    private func startAudio() {
        stopAudio()
        guard isPlaying, let document, let buffer = audioBuffer else { return }
        let start = document.startSample(ofFrame: frame)
        guard start < UInt64(buffer.frameLength) else { return }
        let remaining = AVAudioFrameCount(UInt64(buffer.frameLength) - start)
        guard let segment = AVAudioPCMBuffer(pcmFormat: buffer.format, frameCapacity: remaining),
              let source = buffer.floatChannelData?[0], let target = segment.floatChannelData?[0]
        else { return }
        target.update(from: source + Int(start), count: Int(remaining))
        segment.frameLength = remaining
        do {
            try AVAudioSession.sharedInstance().setCategory(.playback, mode: .default)
            try AVAudioSession.sharedInstance().setActive(true)
            if !engine.isRunning { try engine.start() }
        } catch {
            audioStatus = .unavailable("Äänilaitetta ei voitu käynnistää: \(error.localizedDescription)")
            return
        }
        audioPlayer.scheduleBuffer(segment, at: nil)
        audioPlayer.play()
        audioClockStart = start
        // The audio clock starts with the first rendered sample, not now.
        playStart = (CACurrentMediaTime(), frame)
    }

    private func stopAudio() {
        audioClockStart = nil
        audioPlayer.stop()
        if engine.isRunning { engine.pause() }
    }

    /// Sample being heard, once the player has rendered audio.
    private func heardSample() -> UInt64? {
        guard let start = audioClockStart,
              let nodeTime = audioPlayer.lastRenderTime,
              let playerTime = audioPlayer.playerTime(forNodeTime: nodeTime),
              playerTime.sampleRate > 0
        else { return nil }
        let played = max(0, Double(playerTime.sampleTime)) * 24_000 / playerTime.sampleRate
        return start + UInt64(played)
    }

    /// Frame from the audio clock, or from elapsed wall time without audio, so
    /// slow rendering drops frames, never time.
    private func tick() {
        guard let document, let start = playStart, frameCount > 0 else { return pause() }
        let target: UInt64
        if audioClockStart != nil {
            guard let sample = heardSample() else { return }
            target = document.frame(atSample: sample)
        } else {
            let elapsed = CACurrentMediaTime() - start.time
            target = start.frame + UInt64(max(0, elapsed) * Double(document.fps))
        }
        if target >= frameCount {
            frame = frameCount - 1
            requestRender()
            pause()
        } else if target != frame {
            frame = target
            requestRender()
        }
    }

    /// Prepare the current scene's audio in the background. One preparation runs
    /// at a time; switching scenes meanwhile prepares only the newest one.
    private func loadAudio() {
        stopAudio()
        audioBuffer = nil
        guard document != nil else { audioStatus = .none; return }
        audioStatus = .loading
        audioWanted = true
        drainAudio()
    }

    private func drainAudio() {
        guard !audioLoading, audioWanted, let document, let scene else { return }
        audioWanted = false
        audioLoading = true
        audioQueue.async { [weak self] in
            let result = Result { try document.sceneAudio(scene: scene.index) }
            DispatchQueue.main.async {
                guard let self else { return }
                self.audioLoading = false
                if self.document === document, self.scene == scene, !self.audioWanted {
                    switch result {
                    case .success(let buffer?):
                        self.audioBuffer = buffer
                        self.audioStatus = .ready
                    case .success(nil):
                        self.audioStatus = .none
                    case .failure(let error):
                        self.audioStatus = .unavailable(error.localizedDescription)
                    }
                    // Frames of this scene now carry the prepared mouths.
                    self.requestRender()
                    if self.isPlaying { self.startAudio() }
                    if self.playWhenReady { self.playWhenReady = false; self.play() }
                }
                self.drainAudio()
            }
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
