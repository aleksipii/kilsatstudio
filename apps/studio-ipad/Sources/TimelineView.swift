import SwiftUI

/// Touch timeline below the preview: one track per character, four lanes
/// (movement; facing, pose and expression; gestures and props; dialogue).
///
/// Tap an event to select it and jump to it; drag the selected event to move
/// it, or its end handle to change its length. Positions snap to frames in the
/// shared core while dragging, the same as on the Mac, and every edit becomes
/// an unsaved change to the screenplay that Undo reverts.
struct TimelinePanel: View {
    @EnvironmentObject private var player: PlayerModel
    @State private var zoom: CGFloat = 1
    @GestureState private var pinch: CGFloat = 1

    var body: some View {
        VStack(spacing: 0) {
            TimelineToolbar(zoom: $zoom)
            Divider()
            if let timeline = player.timeline {
                GeometryReader { proxy in
                    let width = max((proxy.size.width - TimelineMetrics.labelWidth - 24) * zoom * pinch, 120)
                    ScrollView(.vertical) {
                        HStack(alignment: .top, spacing: 0) {
                            TrackLabels(actors: timeline.actors)
                            ScrollView(.horizontal) {
                                TimelineTracks(timeline: timeline, trackWidth: width)
                                    .padding(.trailing, 24)
                            }
                            // An explicit width: inside the vertical scroll view the
                            // horizontal one would otherwise grow and center its tracks.
                            .frame(width: max(proxy.size.width - TimelineMetrics.labelWidth, 1), alignment: .leading)
                        }
                    }
                    .simultaneousGesture(
                        MagnificationGesture()
                            .updating($pinch) { value, state, _ in state = value }
                            .onEnded { zoom = TimelineMetrics.clampZoom(zoom * $0) })
                }
            } else {
                Text("Aikajana näkyy, kun käsikirjoitus avautuu.")
                    .foregroundStyle(.secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            if let message = player.timelineError ?? player.timelineBlockedReason {
                Label(message, systemImage: player.timelineError == nil ? "hourglass" : "exclamationmark.triangle.fill")
                    .font(.footnote).lineLimit(3)
                    .foregroundStyle(player.timelineError == nil ? Color.secondary : Color.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 12).padding(.vertical, 6)
                    .background(player.timelineError == nil ? Color.clear : Color.red.opacity(0.08))
                    .onTapGesture { player.timelineError = nil }
            }
            if let timeline = player.timeline, let index = player.selectedEvent,
               timeline.events.indices.contains(index) {
                Divider()
                EventInspector(event: timeline.events[index], timeline: timeline)
                    .id(timeline.events[index])
            }
        }
    }
}

enum TimelineMetrics {
    static let labelWidth: CGFloat = 76
    static let rulerHeight: CGFloat = 24
    static let laneHeight: CGFloat = 26
    static let lanes = 4
    static var rowHeight: CGFloat { CGFloat(lanes) * laneHeight + 8 }
    static func clampZoom(_ zoom: CGFloat) -> CGFloat { min(max(zoom, 1), 16) }

    static func lane(_ kind: String) -> Int {
        switch kind {
        case "move": return 0
        case "face", "pose", "emotion": return 1
        case "gesture", "hold": return 2
        default: return 3
        }
    }

    static func color(_ kind: String) -> Color {
        switch kind {
        case "move": return .blue
        case "face", "pose": return .teal
        case "emotion": return .orange
        case "gesture": return .green
        case "hold": return .brown
        default: return .purple
        }
    }
}

private struct TimelineToolbar: View {
    @EnvironmentObject private var player: PlayerModel
    @Binding var zoom: CGFloat

    var body: some View {
        HStack(spacing: 14) {
            Text("AIKAJANA").font(.caption.bold()).foregroundStyle(.secondary)
            addMenu
            Button { player.undo() } label: { Label("Kumoa", systemImage: "arrow.uturn.backward") }
                .disabled(!player.canUndo)
                .keyboardShortcut("z", modifiers: .command)
            Button { player.redo() } label: { Label("Tee uudelleen", systemImage: "arrow.uturn.forward") }
                .disabled(!player.canRedo)
                .keyboardShortcut("z", modifiers: [.command, .shift])
            Spacer(minLength: 4)
            Button { zoom = TimelineMetrics.clampZoom(zoom / 1.5) } label: {
                Label("Loitonna", systemImage: "minus.magnifyingglass")
            }
            .disabled(zoom <= 1)
            Button { zoom = TimelineMetrics.clampZoom(zoom * 1.5) } label: {
                Label("Lähennä", systemImage: "plus.magnifyingglass")
            }
            .disabled(zoom >= 16)
        }
        .labelStyle(.iconOnly)
        .imageScale(.large)
        .padding(.horizontal, 12).padding(.vertical, 6)
        .background(.bar)
    }

    private static let kinds: [(String, String)] = [
        ("move", "Liike"), ("gesture", "Ele"), ("emotion", "Ilme"), ("face", "Suunta"), ("speak", "Repliikki"),
    ]

    /// Adds an event at the playhead for the selected event's character, or any character.
    private var addMenu: some View {
        Menu {
            let actors = player.timeline?.actors ?? []
            let selectedActor = player.selectedEvent.flatMap { index in
                player.timeline?.events.first { $0.index == index }?.actor
            }
            let ordered = selectedActor.map { first in [first] + actors.filter { $0 != first } } ?? actors
            ForEach(ordered, id: \.self) { actor in
                Section(actor) {
                    ForEach(Self.kinds, id: \.0) { kind, label in
                        Button(label) { player.insertEvent(kind: kind, actor: actor) }
                    }
                }
            }
        } label: {
            Label("Lisää tapahtuma toistopäähän", systemImage: "plus.circle")
        }
        .disabled(!player.canEditTimeline || (player.timeline?.actors.isEmpty ?? true))
    }
}

private struct TrackLabels: View {
    let actors: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Color.clear.frame(height: TimelineMetrics.rulerHeight)
            ForEach(actors, id: \.self) { actor in
                Text(actor).font(.callout.bold()).lineLimit(2).minimumScaleFactor(0.7)
                    .padding(.horizontal, 8).padding(.top, 6)
                    .frame(width: TimelineMetrics.labelWidth, height: TimelineMetrics.rowHeight, alignment: .topLeading)
                    .overlay(alignment: .bottom) { Divider() }
            }
        }
        .background(Color(uiColor: .secondarySystemBackground))
    }
}

/// The ruler, tracks, events and playhead at `trackWidth` points for the scene.
private struct TimelineTracks: View {
    @EnvironmentObject private var player: PlayerModel
    let timeline: AnimationDocument.Timeline
    let trackWidth: CGFloat

    private var height: CGFloat {
        TimelineMetrics.rulerHeight + CGFloat(timeline.actors.count) * TimelineMetrics.rowHeight
    }

    var body: some View {
        ZStack(alignment: .topLeading) {
            Ruler(duration: timeline.duration, width: trackWidth, height: height)
                .contentShape(Rectangle())
                // Tap or drag on an empty spot to move the playhead.
                .gesture(DragGesture(minimumDistance: 0).onChanged { seek(to: $0.location.x) })
            ForEach(timeline.events) { event in
                if let row = timeline.actors.firstIndex(of: event.actor) {
                    EventBar(event: event, timeline: timeline, trackWidth: trackWidth, row: row)
                        .zIndex(player.selectedEvent == event.index ? 1 : 0)
                }
            }
            Rectangle().fill(Color.red).frame(width: 2, height: height)
                .offset(x: x(player.playheadSeconds) - 1)
                .allowsHitTesting(false)
                .zIndex(2)
        }
        .frame(width: trackWidth, height: height, alignment: .topLeading)
    }

    private func x(_ seconds: Double) -> CGFloat {
        guard timeline.duration > 0 else { return 0 }
        return CGFloat(min(max(seconds / timeline.duration, 0), 1)) * trackWidth
    }

    private func seek(to x: CGFloat) {
        guard trackWidth > 0, timeline.fps > 0 else { return }
        player.pause()
        let seconds = Double(min(max(x / trackWidth, 0), 1)) * timeline.duration
        player.seek(to: UInt64((seconds * Double(timeline.fps)).rounded(.down)))
    }
}

private struct Ruler: View {
    let duration: Double
    let width: CGFloat
    let height: CGFloat

    var body: some View {
        Canvas { context, size in
            guard duration > 0 else { return }
            let step = Self.step(duration: duration, width: width)
            var time = 0.0
            while time <= duration + 1e-9 {
                let x = CGFloat(time / duration) * width
                var line = Path()
                line.move(to: CGPoint(x: x, y: TimelineMetrics.rulerHeight - 6))
                line.addLine(to: CGPoint(x: x, y: size.height))
                context.stroke(line, with: .color(.secondary.opacity(0.25)), lineWidth: 1)
                context.draw(Text(Self.label(time)).font(.caption2.monospacedDigit()).foregroundColor(.secondary),
                             at: CGPoint(x: x + 3, y: 2), anchor: .topLeading)
                time += step
            }
            for row in 0...Int(((size.height - TimelineMetrics.rulerHeight) / TimelineMetrics.rowHeight).rounded()) {
                let y = TimelineMetrics.rulerHeight + CGFloat(row) * TimelineMetrics.rowHeight
                var line = Path()
                line.move(to: CGPoint(x: 0, y: y))
                line.addLine(to: CGPoint(x: size.width, y: y))
                context.stroke(line, with: .color(.secondary.opacity(0.35)), lineWidth: 1)
            }
        }
        .frame(width: width, height: height)
    }

    /// A tick every 0.25, 0.5, 1, 2, 5, 10… seconds, at least 56 points apart.
    static func step(duration: Double, width: CGFloat) -> Double {
        let perSecond = Double(width) / duration
        return [0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300].first { $0 * perSecond >= 56 } ?? 600
    }

    static func label(_ seconds: Double) -> String {
        seconds.truncatingRemainder(dividingBy: 1) == 0
            ? String(format: "%.0f s", seconds) : String(format: "%.2f", seconds)
    }
}

/// One event. Point events and lines (whose length comes from their audio)
/// are chips that start at their time; moves and gestures span their length.
private struct EventBar: View {
    @EnvironmentObject private var player: PlayerModel
    let event: AnimationDocument.Timeline.Event
    let timeline: AnimationDocument.Timeline
    let trackWidth: CGFloat
    let row: Int
    /// Frame-snapped start and length while dragging.
    @State private var preview: Snap?

    private struct Snap: Equatable { let at: Double; let duration: Double }

    private var selected: Bool { player.selectedEvent == event.index }

    var body: some View {
        let shown = preview ?? Snap(at: event.at, duration: event.duration ?? 0)
        let scale = timeline.duration > 0 ? trackWidth / CGFloat(timeline.duration) : 0
        let width = event.resizable
            ? max(CGFloat(shown.duration) * scale, 16)
            : (event.kind == "speak" ? 150 : 84)
        let color = TimelineMetrics.color(event.kind)
        let y = TimelineMetrics.rulerHeight + CGFloat(row) * TimelineMetrics.rowHeight + 4
            + CGFloat(TimelineMetrics.lane(event.kind)) * TimelineMetrics.laneHeight

        HStack(spacing: 4) {
            Rectangle().fill(color).frame(width: 3)
            Text(EventLabels.title(event)).font(.caption).lineLimit(1)
            Spacer(minLength: 0)
        }
        .padding(.trailing, event.resizable && selected ? 18 : 4)
        .frame(width: width, height: TimelineMetrics.laneHeight - 4)
        .background(color.opacity(selected ? 0.45 : 0.22), in: RoundedRectangle(cornerRadius: 5))
        .overlay {
            RoundedRectangle(cornerRadius: 5)
                .strokeBorder(selected ? Color.accentColor : color.opacity(0.6), lineWidth: selected ? 2 : 1)
        }
        .overlay(alignment: .trailing) {
            if event.resizable && selected {
                Image(systemName: "arrow.left.and.right")
                    .font(.caption2.bold())
                    .frame(width: 22, height: TimelineMetrics.laneHeight - 4)
                    .background(Color.accentColor.opacity(0.85), in: RoundedRectangle(cornerRadius: 5))
                    .foregroundStyle(.white)
                    .contentShape(Rectangle())
                    .gesture(drag(resize: true))
                    .accessibilityLabel("Muuta pituutta")
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { select() }
        // Only the selected event moves, so a stray swipe scrolls instead.
        .gesture(drag(resize: false), including: selected ? .all : .subviews)
        .offset(x: CGFloat(shown.at) * scale, y: y)
        .onChange(of: event) { _ in preview = nil }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(event.actor): \(EventLabels.title(event)), \(String(format: "%.2f", event.at)) s")
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    }

    private func select() {
        player.timelineError = nil
        player.selectedEvent = event.index
        player.pause()
        if timeline.fps > 0 {
            player.seek(to: UInt64((event.at * Double(timeline.fps)).rounded()))
        }
    }

    private func drag(resize: Bool) -> some Gesture {
        DragGesture(minimumDistance: 4)
            .onChanged { value in
                guard player.canEditTimeline else { return }
                if let snap = player.snapped(event: event.index, delta: delta(value), resize: resize) {
                    preview = Snap(at: snap.at, duration: snap.duration)
                }
            }
            .onEnded { value in
                guard player.canEditTimeline else { preview = nil; return }
                // The preview stays until the edited timeline arrives.
                if !player.moveEvent(event.index, by: delta(value), resize: resize) { preview = nil }
            }
    }

    private func delta(_ value: DragGesture.Value) -> Double {
        guard trackWidth > 0 else { return 0 }
        return Double(value.translation.width / trackWidth) * timeline.duration
    }
}

/// Selected event: its value, start and length, copy and delete.
private struct EventInspector: View {
    @EnvironmentObject private var player: PlayerModel
    let event: AnimationDocument.Timeline.Event
    let timeline: AnimationDocument.Timeline
    @State private var text = ""
    @State private var confirmDelete = false

    private var frame: Double { timeline.fps > 0 ? 1 / Double(timeline.fps) : 0 }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 14) {
                HStack(spacing: 6) {
                    Circle().fill(TimelineMetrics.color(event.kind)).frame(width: 10, height: 10)
                    Text("\(event.actor) · \(EventLabels.kind(event.kind))").font(.callout.bold()).lineLimit(1)
                }
                .fixedSize()
                valueControl
                Spacer(minLength: 0)
                Button { player.duplicateEvent(event.index) } label: {
                    Label("Kopioi toistopäähän", systemImage: "plus.square.on.square")
                }
                .labelStyle(.iconOnly)
                Button(role: .destructive) { confirmDelete = true } label: { Label("Poista", systemImage: "trash") }
                    .labelStyle(.iconOnly)
                    .keyboardShortcut(.delete, modifiers: [])
            }
            HStack(spacing: 18) {
                HStack(spacing: 8) {
                    Text(String(format: "Alku %.2f s", event.at)).font(.callout.monospacedDigit()).fixedSize()
                    Button { player.moveEvent(event.index, by: -frame) } label: { Image(systemName: "chevron.left") }
                        .accessibilityLabel("Ruutu aiemmaksi")
                    Button { player.moveEvent(event.index, by: frame) } label: { Image(systemName: "chevron.right") }
                        .accessibilityLabel("Ruutu myöhemmäksi")
                    Button("Toistopäähän") { player.moveEvent(event.index, by: player.playheadSeconds - event.at) }
                        .fixedSize()
                }
                if let duration = event.duration {
                    HStack(spacing: 8) {
                        Text(String(format: "Pituus %.2f s", duration)).font(.callout.monospacedDigit()).fixedSize()
                        Button { player.moveEvent(event.index, by: -frame, resize: true) } label: { Image(systemName: "minus") }
                            .accessibilityLabel("Ruutu lyhyemmäksi")
                        Button { player.moveEvent(event.index, by: frame, resize: true) } label: { Image(systemName: "plus") }
                            .accessibilityLabel("Ruutu pidemmäksi")
                    }
                }
            }
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12).padding(.vertical, 8)
        .background(.bar)
        .onAppear { text = event.value }
        .confirmationDialog("Poistetaanko tapahtuma?", isPresented: $confirmDelete, titleVisibility: .visible) {
            Button("Poista", role: .destructive) { player.removeEvent(event.index) }
        }
        .disabled(!player.canEditTimeline)
    }

    @ViewBuilder private var valueControl: some View {
        let choices = EventLabels.choices(for: event.kind, in: timeline.choices)
        if !choices.isEmpty {
            Picker(EventLabels.kind(event.kind), selection: Binding(
                get: { event.value },
                set: { player.setEventValue(event.index, to: $0) })
            ) {
                ForEach(choices + (choices.contains(event.value) ? [] : [event.value]), id: \.self) {
                    Text(EventLabels.value($0, kind: event.kind)).tag($0)
                }
            }
            .pickerStyle(.menu)
            .fixedSize()
        } else {
            TextField(event.kind == "hold" ? "Esine (tyhjä laskee esineen)" : "Repliikki", text: $text)
                .textFieldStyle(.roundedBorder)
                .frame(minWidth: 160, maxWidth: 360)
                .submitLabel(.done)
                .onSubmit { if text != event.value { player.setEventValue(event.index, to: text) } }
        }
    }
}

/// Finnish names for event kinds and values, as on the Mac timeline.
enum EventLabels {
    static func kind(_ kind: String) -> String {
        switch kind {
        case "move": return "Liike"
        case "gesture": return "Ele"
        case "emotion": return "Ilme"
        case "face": return "Suunta"
        case "pose": return "Asento"
        case "hold": return "Esine"
        case "speak": return "Repliikki"
        default: return kind
        }
    }

    static func title(_ event: AnimationDocument.Timeline.Event) -> String {
        if event.kind == "hold" && event.value.isEmpty { return "laskee esineen" }
        return value(event.value, kind: event.kind)
    }

    static func choices(for kind: String, in choices: AnimationDocument.Timeline.Choices) -> [String] {
        switch kind {
        case "gesture": return choices.gesture
        case "emotion": return choices.emotion
        case "face": return choices.face
        case "pose": return choices.pose
        case "move": return choices.gait
        default: return []
        }
    }

    static func value(_ value: String, kind: String) -> String {
        let names: [String: String]
        switch kind {
        case "gesture": names = gestures
        case "emotion": names = emotions
        case "face": names = facings
        case "pose": names = poses
        case "move": names = gaits
        default: return value
        }
        return names[value] ?? value
    }

    private static let gestures = [
        "wave": "Vilkutus", "point": "Osoitus", "nod": "Nyökkäys", "shrug": "Olankohautus",
        "shake_head": "Päänpudistus", "facepalm": "Facepalm", "arms_crossed": "Kädet puuskassa",
        "thumbs_up": "Peukku ylös", "bow": "Kumarrus", "jump": "Hyppy", "clap": "Taputus",
        "think": "Mietintä", "stumble": "Kompastuminen",
    ]
    private static let emotions = [
        "neutral": "Neutraali", "happy": "Iloinen", "sad": "Surullinen",
        "surprised": "Yllättynyt", "angry": "Vihainen",
    ]
    private static let facings = [
        "front": "Edestä", "three_quarter_left": "3/4 vasemmalle", "three_quarter_right": "3/4 oikealle",
        "left": "Profiili vasemmalle", "right": "Profiili oikealle", "back": "Selkä katsojaan",
    ]
    private static let poses = [
        "sit": "Istuu", "stand": "Seisoo", "crouch": "Kyykky", "kneel": "Polvillaan",
        "lie": "Makaa", "fall": "Kaatuu",
    ]
    private static let gaits = [
        "walk": "Kävely", "jog": "Hölkkä", "run": "Juoksu", "stroll": "Löntystely",
        "sneak": "Hiipiminen", "tiptoe": "Varpaillaan", "limp": "Ontuminen", "sprint": "Pyrähdys",
        "backwards": "Takaperin", "leap": "Hyppy", "carry": "Kantaa",
    ]
}
