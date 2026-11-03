import AVFoundation
import CoreGraphics
import Foundation
import KoetaCore

/// Error message from the shared Rust core, shown to the user as is.
struct CoreError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// Takes ownership of a C string from the core and frees it.
private func takeString(_ pointer: UnsafeMutablePointer<CChar>?) -> String? {
    guard let pointer else { return nil }
    defer { koeta_string_free(pointer) }
    return String(cString: pointer)
}

/// An opened MotionScript screenplay or .kilsat project.
///
/// All parsing, evaluation, audio mixing and rendering happen in the shared Rust
/// core, so the iPad shows the same frames and plays the same mix as export on
/// the Mac. The core locks its own caches, so rendering and audio preparation
/// may run on different background queues.
final class AnimationDocument: @unchecked Sendable {
    struct Scene: Identifiable, Hashable {
        let index: UInt32
        let id: String
        let frameCount: UInt64
    }

    private let handle: OpaquePointer
    let title: String
    let fps: UInt32
    let scenes: [Scene]

    /// Opens a file by path. Image assets load here; imported audio is read
    /// later from the same folder, so keep read access to it while open.
    init(url: URL) throws {
        var error: UnsafeMutablePointer<CChar>?
        let opened = url.path.withCString { koeta_document_open_path($0, &error) }
        guard let opened else {
            throw CoreError(message: takeString(error) ?? "Tiedostoa ei voitu avata.")
        }
        handle = opened
        title = takeString(koeta_document_title(opened)) ?? ""
        fps = koeta_document_fps(opened)
        var scenes: [Scene] = []
        for index in 0..<koeta_document_scene_count(opened) {
            var error: UnsafeMutablePointer<CChar>?
            let frames = koeta_document_frame_count(opened, index, &error)
            if let message = takeString(error) {
                koeta_document_free(opened)
                throw CoreError(message: message)
            }
            let id = takeString(koeta_document_scene_id(opened, index)) ?? "\(index + 1)"
            scenes.append(Scene(index: index, id: id, frameCount: frames))
        }
        self.scenes = scenes
    }

    deinit {
        koeta_document_free(handle)
    }

    /// Render one frame; longest side at most `maxSide` (1...1024) pixels.
    func render(scene: UInt32, frame: UInt64, maxSide: UInt32) throws -> CGImage {
        var output = KoetaFrame(rgba: nil, len: 0, width: 0, height: 0)
        var error: UnsafeMutablePointer<CChar>?
        guard koeta_render_frame(handle, scene, frame, maxSide, &output, &error) else {
            throw CoreError(message: takeString(error) ?? "Ruutua ei voitu piirtää.")
        }
        defer { koeta_frame_free(&output) }
        let pixels = Data(bytes: output.rgba, count: output.len)
        guard let provider = CGDataProvider(data: pixels as CFData),
              let image = CGImage(
                width: Int(output.width), height: Int(output.height),
                bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: Int(output.width) * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent)
        else {
            throw CoreError(message: "Ruudun kuvaa ei voitu muodostaa.")
        }
        return image
    }

    /// The scene's mix of imported dialogue and audio cues as 24 kHz mono, or nil
    /// when the scene has no audio. Never synthesizes speech: a line without its
    /// WAV file throws. Blocking, so call it off the main thread. Afterwards the
    /// scene's rendered frames show mouths from the same manifest as export.
    func sceneAudio(scene: UInt32) throws -> AVAudioPCMBuffer? {
        var audio = KoetaAudio(samples: nil, len: 0, sample_rate: 0)
        var error: UnsafeMutablePointer<CChar>?
        guard koeta_scene_audio(handle, scene, &audio, &error) else {
            throw CoreError(message: takeString(error) ?? "Ääntä ei voitu valmistella.")
        }
        defer { koeta_audio_free(&audio) }
        guard let samples = audio.samples, audio.len > 0 else { return nil }
        guard let format = AVAudioFormat(
                commonFormat: .pcmFormatFloat32, sampleRate: Double(audio.sample_rate),
                channels: 1, interleaved: false),
              let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(audio.len)),
              let channel = buffer.floatChannelData?[0]
        else {
            throw CoreError(message: "Äänipuskuria ei voitu varata.")
        }
        channel.update(from: samples, count: audio.len)
        buffer.frameLength = AVAudioFrameCount(audio.len)
        return buffer
    }

    /// Frame that shows `sample` on the shared 24 kHz clock.
    func frame(atSample sample: UInt64) -> UInt64 {
        koeta_document_frame_at_sample(handle, sample)
    }

    /// First sample of `frame` on the shared 24 kHz clock.
    func startSample(ofFrame frame: UInt64) -> UInt64 {
        koeta_document_frame_start_sample(handle, frame)
    }
}
