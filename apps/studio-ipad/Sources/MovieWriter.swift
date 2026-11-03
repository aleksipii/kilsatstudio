import Accelerate
import AVFoundation
import CoreMedia
import CoreVideo

/// Encodes the shared core's export frames and mix into an H.264/AAC MP4.
///
/// The Mac pipes the same frames and mix to FFmpeg; iPadOS cannot run FFmpeg,
/// so AVFoundation encodes here. Frame `n` is shown at `n / fps` seconds and the
/// audio starts at sample 0 of the 24 kHz mix, the same timing as the Mac
/// export. One frame is in memory at a time.
enum MovieWriter {
    /// Largest side H.264 encoders on iPad accept.
    static let maxSide = 4096
    private static let sampleRate = 24_000

    /// Blocking: call on a background queue. `progress` gets the completed
    /// fraction from the encoder's queue; `isCancelled` is polled between frames.
    static func write(
        _ frames: MovieFrames, to url: URL,
        isCancelled: @escaping @Sendable () -> Bool, progress: @escaping (Double) -> Void
    ) throws {
        let width = frames.width
        let height = frames.height
        guard width > 0, height > 0, width % 2 == 0, height % 2 == 0,
              width <= maxSide, height <= maxSide, frames.fps > 0, frames.frameCount > 0
        else {
            throw CoreError(message: "Videon leveyden ja korkeuden on oltava parillisia ja enintään \(maxSide) pikseliä (nyt \(width)×\(height)).")
        }
        let samples = try frames.audio()
        if isCancelled() { throw cancelled }
        try? FileManager.default.removeItem(at: url)

        let writer = try AVAssetWriter(outputURL: url, fileType: .mp4)
        writer.shouldOptimizeForNetworkUse = true
        let pixels = Double(width * height)
        let video = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: width,
            AVVideoHeightKey: height,
            AVVideoColorPropertiesKey: [
                AVVideoColorPrimariesKey: AVVideoColorPrimaries_ITU_R_709_2,
                AVVideoTransferFunctionKey: AVVideoTransferFunction_ITU_R_709_2,
                AVVideoYCbCrMatrixKey: AVVideoYCbCrMatrix_ITU_R_709_2,
            ],
            AVVideoCompressionPropertiesKey: [
                // About 0.15 bits per pixel per frame: clean lines at 1080p24 ≈ 7.5 Mbit/s.
                AVVideoAverageBitRateKey: Int(pixels * Double(frames.fps) * 0.15),
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                AVVideoExpectedSourceFrameRateKey: Int(frames.fps),
                AVVideoMaxKeyFrameIntervalKey: Int(frames.fps) * 2,
            ],
        ])
        video.expectsMediaDataInRealTime = false
        let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: video, sourcePixelBufferAttributes: [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: width,
            kCVPixelBufferHeightKey as String: height,
        ])
        guard writer.canAdd(video) else { throw CoreError(message: "Videoraitaa ei voitu luoda.") }
        writer.add(video)

        var audio: AVAssetWriterInput?
        if !samples.isEmpty {
            let input = AVAssetWriterInput(mediaType: .audio, outputSettings: [
                AVFormatIDKey: kAudioFormatMPEG4AAC,
                AVSampleRateKey: sampleRate,
                AVNumberOfChannelsKey: 1,
                // AAC-LC at 24 kHz mono accepts at most about 64 kbit/s.
                AVEncoderBitRateKey: 64_000,
            ])
            input.expectsMediaDataInRealTime = false
            guard writer.canAdd(input) else { throw CoreError(message: "Ääniraitaa ei voitu luoda.") }
            writer.add(input)
            audio = input
        }

        guard writer.startWriting() else { throw failure(writer) }
        writer.startSession(atSourceTime: .zero)
        let format = try audioFormat()
        let state = WriteState()
        let group = DispatchGroup()

        // Each track pulls on its own queue when the writer asks for more, and
        // the writer interleaves them; pushing both from one loop can stall.
        group.enter()
        var nextFrame: UInt64 = 0
        video.requestMediaDataWhenReady(on: DispatchQueue(label: "studio.kilsat.export.video")) {
            while video.isReadyForMoreMediaData {
                if nextFrame == frames.frameCount || state.failed || isCancelled() {
                    video.markAsFinished()
                    group.leave()
                    return
                }
                do {
                    guard let pool = adaptor.pixelBufferPool else { throw failure(writer) }
                    let buffer = try frames.withFrame(nextFrame) { rgba in
                        try pixelBuffer(rgba, width: width, height: height, pool: pool)
                    }
                    let time = CMTime(value: CMTimeValue(nextFrame), timescale: CMTimeScale(frames.fps))
                    guard adaptor.append(buffer, withPresentationTime: time) else { throw failure(writer) }
                    nextFrame += 1
                    progress(Double(nextFrame) / Double(frames.frameCount))
                } catch {
                    state.fail(error)
                }
            }
        }
        if let audio {
            group.enter()
            var written = 0
            audio.requestMediaDataWhenReady(on: DispatchQueue(label: "studio.kilsat.export.audio")) {
                while audio.isReadyForMoreMediaData {
                    if written == samples.count || state.failed || isCancelled() {
                        audio.markAsFinished()
                        group.leave()
                        return
                    }
                    do {
                        let count = min(samples.count - written, sampleRate / 10)
                        let buffer = try audioBuffer(samples, from: written, count: count, format: format)
                        guard audio.append(buffer) else { throw failure(writer) }
                        written += count
                    } catch {
                        state.fail(error)
                    }
                }
            }
        }

        // A failed or cancelled writer may stop asking for data, so watch it
        // instead of waiting on the tracks alone.
        while group.wait(timeout: .now() + 0.1) == .timedOut {
            if writer.status == .failed { state.fail(failure(writer)) }
            if isCancelled() { state.fail(cancelled) }
            if state.failed { break }
        }
        if isCancelled() { state.fail(cancelled) }
        if let error = state.error {
            writer.cancelWriting()
            try? FileManager.default.removeItem(at: url)
            throw error
        }
        let done = DispatchSemaphore(value: 0)
        writer.finishWriting { done.signal() }
        done.wait()
        guard writer.status == .completed else {
            try? FileManager.default.removeItem(at: url)
            throw failure(writer)
        }
    }

    private static let cancelled = CoreError(message: "Vienti peruttiin.")

    private static func failure(_ writer: AVAssetWriter) -> CoreError {
        CoreError(message: "Videon koodaus epäonnistui: \(writer.error?.localizedDescription ?? "tuntematon virhe").")
    }

    /// The first error from either track, shared by both queues.
    private final class WriteState: @unchecked Sendable {
        private let lock = NSLock()
        private var first: Error?
        var error: Error? { lock.withLock { first } }
        var failed: Bool { error != nil }
        func fail(_ error: Error) { lock.withLock { if first == nil { first = error } } }
    }

    /// The core's opaque RGBA as a BGRA pixel buffer for the encoder.
    private static func pixelBuffer(_ rgba: UnsafeRawBufferPointer, width: Int, height: Int, pool: CVPixelBufferPool) throws -> CVPixelBuffer {
        guard rgba.count == width * height * 4, let base = rgba.baseAddress else {
            throw CoreError(message: "Ruudun koko ei vastaa videon kokoa.")
        }
        var created: CVPixelBuffer?
        guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &created) == kCVReturnSuccess, let buffer = created else {
            throw CoreError(message: "Kuvapuskuria ei voitu varata.")
        }
        CVPixelBufferLockBaseAddress(buffer, [])
        defer { CVPixelBufferUnlockBaseAddress(buffer, []) }
        var source = vImage_Buffer(data: UnsafeMutableRawPointer(mutating: base), height: vImagePixelCount(height),
                                   width: vImagePixelCount(width), rowBytes: width * 4)
        var target = vImage_Buffer(data: CVPixelBufferGetBaseAddress(buffer), height: vImagePixelCount(height),
                                   width: vImagePixelCount(width), rowBytes: CVPixelBufferGetBytesPerRow(buffer))
        // RGBA → BGRA.
        let map: [UInt8] = [2, 1, 0, 3]
        guard vImagePermuteChannels_ARGB8888(&source, &target, map, vImage_Flags(kvImageNoFlags)) == kvImageNoError else {
            throw CoreError(message: "Ruudun värijärjestystä ei voitu muuntaa.")
        }
        return buffer
    }

    private static func audioFormat() throws -> CMAudioFormatDescription {
        var description = AudioStreamBasicDescription(
            mSampleRate: Float64(sampleRate), mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
            mBytesPerPacket: 4, mFramesPerPacket: 1, mBytesPerFrame: 4,
            mChannelsPerFrame: 1, mBitsPerChannel: 32, mReserved: 0)
        var format: CMAudioFormatDescription?
        guard CMAudioFormatDescriptionCreate(
                allocator: kCFAllocatorDefault, asbd: &description, layoutSize: 0, layout: nil,
                magicCookieSize: 0, magicCookie: nil, extensions: nil, formatDescriptionOut: &format) == noErr,
              let format
        else { throw CoreError(message: "Äänimuotoa ei voitu luoda.") }
        return format
    }

    /// `count` mono float samples from `start`, stamped on the 24 kHz clock.
    private static func audioBuffer(_ samples: [Float], from start: Int, count: Int, format: CMAudioFormatDescription) throws -> CMSampleBuffer {
        let bytes = count * MemoryLayout<Float>.size
        var block: CMBlockBuffer?
        guard CMBlockBufferCreateWithMemoryBlock(
                allocator: kCFAllocatorDefault, memoryBlock: nil, blockLength: bytes,
                blockAllocator: kCFAllocatorDefault, customBlockSource: nil, offsetToData: 0,
                dataLength: bytes, flags: kCMBlockBufferAssureMemoryNowFlag, blockBufferOut: &block) == kCMBlockBufferNoErr,
              let block
        else { throw CoreError(message: "Äänipuskuria ei voitu varata.") }
        let copied = samples.withUnsafeBytes { all in
            CMBlockBufferReplaceDataBytes(
                with: all.baseAddress! + start * MemoryLayout<Float>.size,
                blockBuffer: block, offsetIntoDestination: 0, dataLength: bytes)
        }
        var buffer: CMSampleBuffer?
        guard copied == kCMBlockBufferNoErr,
              CMAudioSampleBufferCreateReadyWithPacketDescriptions(
                allocator: kCFAllocatorDefault, dataBuffer: block, formatDescription: format,
                sampleCount: count, presentationTimeStamp: CMTime(value: CMTimeValue(start), timescale: CMTimeScale(sampleRate)),
                packetDescriptions: nil, sampleBufferOut: &buffer) == noErr,
              let buffer
        else { throw CoreError(message: "Äänipuskuria ei voitu luoda.") }
        return buffer
    }
}
