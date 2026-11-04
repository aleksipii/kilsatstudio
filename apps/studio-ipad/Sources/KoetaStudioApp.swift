import SwiftUI
import UniformTypeIdentifiers

@main
struct KoetaStudioApp: App {
    @StateObject private var player = PlayerModel()
    @StateObject private var exporter = ExportController()
    @State private var launchExportStarted = false
    @State private var launchDraftAppended = false
    @State private var launchTimelineTested = false

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(player)
                .environmentObject(exporter)
                .onOpenURL { url in player.guardUnsaved { player.open(url: url) } }
                .task { openLaunchExample() }
                .onReceive(player.$document.compactMap { $0 }) { exportAtLaunch($0) }
                .onChange(of: exporter.state) { logExport($0) }
        }
    }

    /// `--export` (with `--open-example`) exports the selected scene, or the
    /// whole project with `--export-all`, once it opens, and logs the outcome
    /// (UI smoke tests of the AVFoundation encoder).
    private func exportAtLaunch(_ document: AnimationDocument) {
        let arguments = ProcessInfo.processInfo.arguments
        // `--draft-append TEXT` types TEXT at the end of the screenplay once (editor smoke tests).
        if let index = arguments.firstIndex(of: "--draft-append"), arguments.indices.contains(index + 1),
           !launchDraftAppended {
            launchDraftAppended = true
            let text = arguments[index + 1].replacingOccurrences(of: "\\n", with: "\n")
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { player.draft += text }
            // `--save-draft` then saves it once the preview has taken it.
            if arguments.contains("--save-draft") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                    player.save()
                    print("KOETA_SAVE requested readOnly=\(player.isReadOnly)")
                }
            }
        }
        if arguments.contains("--timeline-test"), !launchTimelineTested {
            launchTimelineTested = true
            testTimeline()
        }
        guard arguments.contains("--export"), !launchExportStarted else { return }
        launchExportStarted = true
        // Let `--scene` select its scene first.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            let base = (player.fileName as NSString).deletingPathExtension
            exporter.start(document: document, scene: arguments.contains("--export-all") ? nil : player.scene,
                           baseName: base)
        }
    }

    /// `--timeline-test`: select, move and resize the first event, add one at the
    /// playhead, then undo and redo, logging the timeline after each step.
    private func testTimeline() {
        func log(_ step: String) {
            let events = player.timeline?.events ?? []
            let first = events.first.map { String(format: "%@ at=%.4f duration=%@", $0.kind, $0.at,
                                                  $0.duration.map { String(format: "%.4f", $0) } ?? "-") } ?? "none"
            print("KOETA_TIMELINE \(step) events=\(events.count) first=\(first) unsaved=\(player.hasUnsavedChanges) error=\(player.timelineError ?? "-")")
        }
        let steps: [(String, () -> Void)] = [
            ("opened", {}),
            ("moved", { player.selectedEvent = 0; player.moveEvent(0, by: 0.51) }),
            ("resized", { player.moveEvent(0, by: 0.26, resize: true) }),
            ("inserted", {
                player.seek(to: 12)
                if let actor = player.timeline?.actors.first { player.insertEvent(kind: "gesture", actor: actor) }
            }),
            ("undone", { player.undo() }),
            ("redone", { player.redo(); player.selectedEvent = 0 }),
        ]
        for (offset, step) in steps.enumerated() {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1 + Double(offset) * 1.5) { step.1() }
            DispatchQueue.main.asyncAfter(deadline: .now() + 2 + Double(offset) * 1.5) { log(step.0) }
        }
    }

    private func logExport(_ state: ExportController.State) {
        guard ProcessInfo.processInfo.arguments.contains("--export") else { return }
        switch state {
        case .finished(let url): print("KOETA_EXPORT finished \(url.path)")
        case .failed(let message): print("KOETA_EXPORT failed \(message)")
        default: break
        }
    }

    /// `--open-example NAME [--scene N] [--play]` opens a bundled example, or a file in the
    /// app's Files folder, at launch, selects scene N and plays (UI smoke tests).
    private func openLaunchExample() {
        let arguments = ProcessInfo.processInfo.arguments
        func value(_ flag: String) -> String? {
            guard let index = arguments.firstIndex(of: flag), arguments.indices.contains(index + 1)
            else { return nil }
            return arguments[index + 1]
        }
        player.playWhenReady = arguments.contains("--play")
        player.showsEditor = arguments.contains("--edit")
        player.showsTimeline = arguments.contains("--timeline") || arguments.contains("--timeline-test")
        if let name = value("--open-example") {
            player.openExample(named: name, scene: value("--scene").flatMap(Int.init))
        }
    }
}

extension UTType {
    static let koetaProject = UTType(importedAs: "studio.kilsat.project", conformingTo: .json)
}
