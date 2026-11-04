import SwiftUI
import UniformTypeIdentifiers

/// Scene list in the sidebar, preview and transport in the detail column.
/// NavigationSplitView collapses to one column in narrow Split View and
/// Slide Over, so the same layout serves iPad mini to the 13-inch iPad Pro.
struct ContentView: View {
    /// What the one file importer picks. SwiftUI supports a single
    /// `fileImporter` per view, so the mode selects its content types.
    enum ImportMode { case file, folder }

    @EnvironmentObject private var player: PlayerModel
    @State private var importing = false
    @State private var importMode = ImportMode.file
    @State private var exporting = false

    var body: some View {
        NavigationSplitView {
            SceneList()
                .navigationTitle(player.document?.title ?? "Koeta Studio")
        } detail: {
            Group {
                if player.document == nil {
                    Welcome(importing: $importing, importMode: $importMode)
                } else {
                    PreviewScreen(editing: player.showsEditor)
                }
            }
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    if player.document != nil {
                        Toggle(isOn: $player.showsEditor) {
                            Label("Muokkaa", systemImage: "pencil.and.list.clipboard")
                        }
                        .toggleStyle(.button)
                        .keyboardShortcut("e", modifiers: .command)
                        Button {
                            exporting = true
                        } label: {
                            Label("Vie video", systemImage: "film")
                        }
                        .keyboardShortcut("e", modifiers: [.command, .shift])
                    }
                    Button {
                        player.guardUnsaved { importMode = .folder; importing = true }
                    } label: {
                        Label("Avaa kansio", systemImage: "folder.badge.plus")
                    }
                    .keyboardShortcut("o", modifiers: [.command, .shift])
                    Button {
                        player.guardUnsaved { importMode = .file; importing = true }
                    } label: {
                        Label("Avaa", systemImage: "folder")
                    }
                    .keyboardShortcut("o", modifiers: .command)
                }
            }
        }
        .fileImporter(isPresented: $importing,
                      allowedContentTypes: importMode == .folder
                          ? [.folder] : [.yaml, .koetaProject, .json, .plainText]) { result in
            switch result {
            case .success(let url):
                importMode == .folder ? player.open(folder: url) : player.open(url: url)
            case .failure(let error): player.errorMessage = error.localizedDescription
            }
        }
        .sheet(isPresented: $exporting) { ExportSheet() }
        .confirmationDialog(
            "Käsikirjoituksessa on tallentamattomia muutoksia.",
            isPresented: Binding(
                get: { player.pendingDiscard != nil },
                set: { if !$0 { player.pendingDiscard = nil } }),
            titleVisibility: .visible
        ) {
            Button("Hylkää muutokset", role: .destructive) {
                let open = player.pendingDiscard
                player.pendingDiscard = nil
                player.revertDraft()
                open?()
            }
            Button("Jatka muokkausta", role: .cancel) {}
        }
        .sheet(isPresented: Binding(
            get: { !player.folderChoices.isEmpty },
            set: { if !$0 { player.cancelFolderChoice() } })
        ) {
            FolderChoice()
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

/// Several project files in an opened folder: pick the one to open.
private struct FolderChoice: View {
    @EnvironmentObject private var player: PlayerModel

    var body: some View {
        NavigationStack {
            List(player.folderChoices, id: \.self) { url in
                Button(url.lastPathComponent) { player.choose(url) }
            }
            .navigationTitle("Valitse projekti")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Peru") { player.cancelFolderChoice() }
                }
            }
        }
    }
}

private struct Welcome: View {
    @EnvironmentObject private var player: PlayerModel
    @Binding var importing: Bool
    @Binding var importMode: ContentView.ImportMode

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "figure.wave").font(.system(size: 64)).foregroundStyle(.tint)
            Text("Koeta Studio").font(.largeTitle.bold())
            Text("Avaa MotionScript-käsikirjoitus (.yaml) tai Koeta-projekti (.kilsat) Tiedostot-sovelluksesta. Jos projektissa on kuvia tai äänitiedostoja, avaa sen kansio, jotta sovellus saa luvan niihin.")
                .multilineTextAlignment(.center).foregroundStyle(.secondary).frame(maxWidth: 460)
            HStack {
                Button("Avaa kansio…") { importMode = .folder; importing = true }
                Button("Avaa tiedosto…") { importMode = .file; importing = true }
            }
            .buttonStyle(.borderedProminent)
            ViewThatFits {
                HStack { examples }
                VStack { examples }
            }
            .buttonStyle(.bordered)
        }
        .padding()
    }

    @ViewBuilder private var examples: some View {
        Button("Esimerkki: Hello Emma") { player.openExample(named: "hello-emma.yaml") }
        Button("Esimerkki: tuotantodemo") { player.openExample(named: "studio-production-demo.kilsat") }
        Button("Esimerkki: kuvallinen jakso") { player.openExample(named: "kilsat-season/ep02.kilsat") }
    }
}

/// The preview, with the screenplay editor beside it (wide windows) or below
/// it (narrow Split View and portrait iPad mini) while editing.
private struct PreviewScreen: View {
    let editing: Bool

    var body: some View {
        GeometryReader { proxy in
            if !editing {
                Player()
            } else if proxy.size.width >= 700 {
                HStack(spacing: 0) {
                    Player().frame(width: proxy.size.width * 0.55)
                    Divider()
                    ScriptEditor()
                }
            } else {
                VStack(spacing: 0) {
                    Player().frame(height: proxy.size.height * 0.45)
                    Divider()
                    ScriptEditor()
                }
            }
        }
    }
}

private struct Player: View {
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
            AudioStatusLine(status: player.audioStatus)
        }
        .padding()
        .background(.bar)
    }
}

/// Whether the scene plays with sound, and why not when it cannot.
private struct AudioStatusLine: View {
    let status: PlayerModel.AudioStatus

    var body: some View {
        switch status {
        case .none:
            EmptyView()
        case .loading:
            Label("Ääntä valmistellaan…", systemImage: "hourglass")
                .font(.footnote).foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
        case .ready:
            Label("Ääni: tuodut äänitiedostot", systemImage: "speaker.wave.2")
                .font(.footnote).foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
        case .unavailable(let message):
            Label(message, systemImage: "speaker.slash")
                .font(.footnote).foregroundStyle(.orange)
                .lineLimit(4)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}
