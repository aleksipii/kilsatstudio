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

    /// `--open-example NAME` opens a bundled example at launch (UI smoke tests).
    private func openLaunchExample() {
        let arguments = ProcessInfo.processInfo.arguments
        if let flag = arguments.firstIndex(of: "--open-example"), arguments.indices.contains(flag + 1) {
            player.openExample(named: arguments[flag + 1])
        }
    }
}

extension UTType {
    static let koetaProject = UTType(importedAs: "studio.kilsat.project", conformingTo: .json)
}
