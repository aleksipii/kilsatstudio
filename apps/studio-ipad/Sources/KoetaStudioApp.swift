import SwiftUI
import UniformTypeIdentifiers

@main
struct KoetaStudioApp: App {
    @StateObject private var player = PlayerModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(player)
                .onOpenURL { player.open(url: $0) }
                .task { openLaunchExample() }
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
        if let name = value("--open-example") {
            player.openExample(named: name, scene: value("--scene").flatMap(Int.init))
        }
    }
}

extension UTType {
    static let koetaProject = UTType(importedAs: "studio.kilsat.project", conformingTo: .json)
}
