import SwiftUI
import UIKit

/// Runs one movie export at a time on its own queue, off the main thread.
@MainActor
final class ExportController: ObservableObject {
    enum State: Equatable {
        case idle
        case preparing
        case encoding(Double)
        case finished(URL)
        case failed(String)
    }

    @Published private(set) var state = State.idle
    private let queue = DispatchQueue(label: "studio.kilsat.export", qos: .userInitiated)
    /// Cancels the running export; the worker polls it between frames.
    private var current: CancelFlag?

    var isRunning: Bool {
        switch state {
        case .preparing, .encoding: return true
        default: return false
        }
    }

    /// Exports `scene`, or every scene when nil, to Files › Koeta Studio › Viennit.
    func start(document: AnimationDocument, scene: AnimationDocument.Scene?, baseName: String) {
        guard !isRunning else { return }
        state = .preparing
        let flag = CancelFlag()
        current = flag
        let name = Self.fileName(baseName: baseName, scene: scene?.id)
        queue.async { [weak self] in
            let result = Result<URL, Error> {
                let frames = try document.beginExport(scene: scene?.index)
                flag.onCancel { frames.cancel() }
                if flag.isCancelled { throw CoreError(message: "Vienti peruttiin.") }
                let folder = try Self.exportFolder()
                let staged = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".mp4")
                defer { try? FileManager.default.removeItem(at: staged) }
                var reported = -1
                try MovieWriter.write(frames, to: staged, isCancelled: { flag.isCancelled }) { fraction in
                    let percent = Int(fraction * 100)
                    guard percent != reported else { return }
                    reported = percent
                    DispatchQueue.main.async { self?.state = .encoding(fraction) }
                }
                let target = folder.appendingPathComponent(name)
                if FileManager.default.fileExists(atPath: target.path) {
                    _ = try FileManager.default.replaceItemAt(target, withItemAt: staged)
                } else {
                    try FileManager.default.moveItem(at: staged, to: target)
                }
                return target
            }
            DispatchQueue.main.async {
                switch result {
                case .success(let url): self?.state = .finished(url)
                case .failure(let error): self?.state = .failed(error.localizedDescription)
                }
            }
        }
    }

    func cancel() {
        current?.cancel()
    }

    func reset() {
        if !isRunning { state = .idle }
    }

    /// Documents/Viennit, visible in Files as Koeta Studio › Viennit.
    nonisolated private static func exportFolder() throws -> URL {
        guard let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else {
            throw CoreError(message: "Sovelluksen kansiota ei löytynyt.")
        }
        let folder = documents.appendingPathComponent("Viennit", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        return folder
    }

    nonisolated static func fileName(baseName: String, scene: String?) -> String {
        let stem = ([baseName, scene ?? "koko"]).joined(separator: "-")
        let safe = stem.map { "/\\:?%*|\"<>".contains($0) ? "_" : $0 }
        return String(safe) + ".mp4"
    }
}

/// Thread-safe cancel flag shared by the UI and the export worker.
final class CancelFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false
    private var handlers: [() -> Void] = []

    var isCancelled: Bool { lock.withLock { cancelled } }

    /// Runs `handler` on cancel, or now if already cancelled.
    func onCancel(_ handler: @escaping () -> Void) {
        let now = lock.withLock { () -> Bool in
            if !cancelled { handlers.append(handler) }
            return cancelled
        }
        if now { handler() }
    }

    func cancel() {
        let run = lock.withLock { () -> [() -> Void] in
            cancelled = true
            defer { handlers = [] }
            return handlers
        }
        run.forEach { $0() }
    }
}

/// Choose what to export, follow progress, then share or find the movie.
struct ExportSheet: View {
    @EnvironmentObject private var player: PlayerModel
    @EnvironmentObject private var exporter: ExportController
    @Environment(\.dismiss) private var dismiss
    @State private var wholeProject = false
    @State private var sharing: SharedMovie?

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Picker("Vietävä", selection: $wholeProject) {
                        Text("Tämä kohtaus: \(player.scene?.id ?? "")").tag(false)
                        Text("Koko projekti (\(player.document?.scenes.count ?? 0) kohtausta)").tag(true)
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                    .disabled(exporter.isRunning)
                } footer: {
                    Text("MP4 (H.264 ja AAC) projektin omassa koossa, samat ruudut ja sama ääni kuin Macin viennissä. Repliikkien äänen on oltava tuotuja WAV-tiedostoja: iPad ei syntetisoi puhetta.")
                }
                if player.hasUnsavedChanges {
                    Section {
                        Label("Vienti käyttää esikatselussa näkyvää muokattua käsikirjoitusta, vaikka sitä ei ole tallennettu.", systemImage: "pencil")
                            .font(.footnote)
                    }
                }
                Section { status }
            }
            .navigationTitle("Vie video")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(exporter.isRunning ? "Peru vienti" : "Sulje") {
                        if exporter.isRunning { exporter.cancel() } else { exporter.reset(); dismiss() }
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Vie") { start() }.disabled(exporter.isRunning || player.document == nil)
                }
            }
            .sheet(item: $sharing) { ShareSheet(items: [$0.url]) }
        }
        .interactiveDismissDisabled(exporter.isRunning)
    }

    @ViewBuilder private var status: some View {
        switch exporter.state {
        case .idle:
            Text("Valmis vietäväksi.").foregroundStyle(.secondary)
        case .preparing:
            ProgressView("Valmistellaan ääntä ja kuvaa…")
        case .encoding(let fraction):
            ProgressView(value: fraction) { Text("Koodataan videota… \(Int(fraction * 100)) %") }
        case .finished(let url):
            VStack(alignment: .leading, spacing: 8) {
                Label("Valmis: \(url.lastPathComponent)", systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                Text("Tallennettu Tiedostot-sovellukseen: Koeta Studio › Viennit.").font(.footnote).foregroundStyle(.secondary)
                Button { sharing = SharedMovie(url: url) } label: { Label("Jaa tai tallenna Kuviin…", systemImage: "square.and.arrow.up") }
            }
        case .failed(let message):
            Label(message, systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
        }
    }

    private func start() {
        guard let document = player.document else { return }
        let base = (player.fileName as NSString).deletingPathExtension
        exporter.start(document: document, scene: wholeProject ? nil : player.scene, baseName: base)
    }
}

/// A finished movie offered to the share sheet.
private struct SharedMovie: Identifiable {
    let url: URL
    var id: URL { url }
}

/// The system share sheet: AirDrop, Save Video to Photos, Save to Files and others.
struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
