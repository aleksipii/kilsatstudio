import SwiftUI
import UniformTypeIdentifiers

/// Scene list in the sidebar, preview and transport in the detail column.
/// NavigationSplitView collapses to one column in narrow Split View and
/// Slide Over, so the same layout serves iPad mini to the 13-inch iPad Pro.
struct ContentView: View {
    @EnvironmentObject private var player: PlayerModel
    @State private var importing = false

    var body: some View {
        NavigationSplitView {
            SceneList()
                .navigationTitle(player.document?.title ?? "Koeta Studio")
        } detail: {
            Group {
                if player.document == nil {
                    Welcome(importing: $importing)
                } else {
                    PreviewScreen()
                }
            }
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        importing = true
                    } label: {
                        Label("Avaa", systemImage: "folder")
                    }
                    .keyboardShortcut("o", modifiers: .command)
                }
            }
        }
        .fileImporter(isPresented: $importing,
                      allowedContentTypes: [.yaml, .koetaProject, .json, .plainText]) { result in
            switch result {
            case .success(let url): player.open(url: url)
            case .failure(let error): player.errorMessage = error.localizedDescription
            }
        }
        .alert("Koeta Studio", isPresented: Binding(
            get: { player.errorMessage != nil },
            set: { if !$0 { player.errorMessage = nil } })
        ) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(player.errorMessage ?? "")
        }
    }
}

private struct SceneList: View {
    @EnvironmentObject private var player: PlayerModel

    var body: some View {
        if let document = player.document {
            List(selection: Binding(
                get: { player.sceneIndex },
                set: { if let index = $0 { player.sceneIndex = index } })
            ) {
                Section(player.fileName) {
                    ForEach(Array(document.scenes.enumerated()), id: \.offset) { offset, scene in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(scene.id).font(.headline)
                            Text(duration(scene, fps: document.fps))
                                .font(.caption).foregroundStyle(.secondary)
                        }
                        .tag(offset)
                    }
                }
            }
        } else {
            Text("Ei avattua tiedostoa").foregroundStyle(.secondary)
        }
    }

    private func duration(_ scene: AnimationDocument.Scene, fps: UInt32) -> String {
        let seconds = fps > 0 ? Double(scene.frameCount) / Double(fps) : 0
        return String(format: "%.1f s · %llu ruutua · %u fps", seconds, scene.frameCount, fps)
    }
}

private struct Welcome: View {
    @EnvironmentObject private var player: PlayerModel
    @Binding var importing: Bool

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "figure.wave").font(.system(size: 64)).foregroundStyle(.tint)
            Text("Koeta Studio").font(.largeTitle.bold())
            Text("Avaa MotionScript-käsikirjoitus (.yaml) tai Koeta-projekti (.kilsat) Tiedostot-sovelluksesta.")
                .multilineTextAlignment(.center).foregroundStyle(.secondary).frame(maxWidth: 420)
            Button("Avaa tiedosto…") { importing = true }.buttonStyle(.borderedProminent)
            HStack {
                Button("Esimerkki: Hello Emma") { player.openExample(named: "hello-emma.yaml") }
                Button("Esimerkki: tuotantodemo") { player.openExample(named: "studio-production-demo.kilsat") }
            }
            .buttonStyle(.bordered)
        }
        .padding()
    }
}

private struct PreviewScreen: View {
    @EnvironmentObject private var player: PlayerModel
    @Environment(\.displayScale) private var displayScale
    @State private var scrubOrigin: UInt64?

    var body: some View {
        VStack(spacing: 0) {
            GeometryReader { proxy in
                ZStack {
                    Color.black
                    if let image = player.image {
                        Image(decorative: image, scale: 1)
                            .resizable().interpolation(.high).scaledToFit()
                    } else {
                        ProgressView().tint(.white)
                    }
                }
                .contentShape(Rectangle())
                // Drag across the picture with a finger or Apple Pencil to scrub.
                .gesture(DragGesture(minimumDistance: 4).onChanged { value in
                    let origin = scrubOrigin ?? player.frame
                    if scrubOrigin == nil { scrubOrigin = origin; player.pause() }
                    let perPoint = Double(player.frameCount) / max(Double(proxy.size.width), 1)
                    let delta = Int64((value.translation.width * perPoint).rounded())
                    let target = max(0, min(Int64(origin) + delta, Int64(player.frameCount) - 1))
                    player.seek(to: UInt64(target))
                }.onEnded { _ in scrubOrigin = nil })
                .onTapGesture { player.togglePlay() }
                .onAppear { updateSide(proxy.size) }
                .onChange(of: proxy.size) { updateSide($0) }
            }
            Transport()
        }
        .navigationTitle(player.scene?.id ?? "")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func updateSide(_ size: CGSize) {
        let pixels = max(size.width, size.height) * displayScale
        player.renderSide = UInt32(min(max(pixels, 64), 1024))
    }
}

private struct Transport: View {
    @EnvironmentObject private var player: PlayerModel

    var body: some View {
        VStack(spacing: 8) {
            Slider(
                value: Binding(
                    get: { Double(player.frame) },
                    set: { player.pause(); player.seek(to: UInt64($0.rounded())) }),
                in: 0...Double(max(player.frameCount, 2) - 1), step: 1)
            HStack(spacing: 24) {
                Button { player.step(-1) } label: { Image(systemName: "backward.frame.fill") }
                    .keyboardShortcut(.leftArrow, modifiers: [])
                Button { player.togglePlay() } label: {
                    Image(systemName: player.isPlaying ? "pause.fill" : "play.fill").font(.title)
                }
                .keyboardShortcut(.space, modifiers: [])
                Button { player.step(1) } label: { Image(systemName: "forward.frame.fill") }
                    .keyboardShortcut(.rightArrow, modifiers: [])
                Spacer()
                Text(player.timecode).font(.body.monospacedDigit()).foregroundStyle(.secondary)
            }
            .buttonStyle(.borderless)
            .imageScale(.large)
        }
        .padding()
        .background(.bar)
    }
}
