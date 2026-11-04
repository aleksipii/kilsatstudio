import SwiftUI
import UIKit

/// MotionScript editor beside the preview. Each pause in typing checks the
/// text with the shared core; valid text updates the preview at once, and
/// a problem shows its kind and line. Saving writes the file.
struct ScriptEditor: View {
    @EnvironmentObject private var player: PlayerModel
    @State private var jumpLine: UInt32?

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                statusLabel.lineLimit(1).layoutPriority(-1)
                Spacer(minLength: 8)
                Button("Peru") { player.revertDraft() }
                    .disabled(!player.hasUnsavedChanges || player.isSaving)
                    .fixedSize()
                Button(player.isReadOnly ? "Tallenna kopio" : "Tallenna") { player.save() }
                    .keyboardShortcut("s", modifiers: .command)
                    .buttonStyle(.borderedProminent)
                    .disabled(!player.canSave)
                    .fixedSize()
            }
            .font(.callout)
            .padding(.horizontal, 12).padding(.vertical, 8)
            .background(.bar)
            CodeTextView(text: $player.draft, jumpLine: $jumpLine)
            if let problem = player.sourceProblem {
                Button { if problem.line > 0 { jumpLine = problem.line } } label: {
                    Label((problem.line > 0 ? "Rivi \(problem.line): " : "") + problem.message
                          + " Esikatselu näyttää viimeisen toimivan version.",
                          systemImage: "exclamationmark.triangle.fill")
                        .font(.footnote).multilineTextAlignment(.leading).lineLimit(5)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(.plain)
                .foregroundStyle(.red)
                .padding(10)
                .background(.red.opacity(0.08))
            }
        }
    }

    @ViewBuilder private var statusLabel: some View {
        if player.isSaving {
            Label("Tallennetaan…", systemImage: "hourglass").foregroundStyle(.secondary)
        } else if player.sourceProblem != nil {
            Label("Virhe", systemImage: "xmark.octagon").foregroundStyle(.red)
        } else if player.hasUnsavedChanges {
            Label("Tallentamatta", systemImage: "circle.fill").foregroundStyle(.orange)
        } else {
            Label("Tallennettu", systemImage: "checkmark.circle").foregroundStyle(.secondary)
        }
    }
}

/// UITextView for YAML: no smart quotes, dashes, autocorrection or capitals,
/// which would silently change the screenplay. `jumpLine` selects that line.
struct CodeTextView: UIViewRepresentable {
    @Binding var text: String
    @Binding var jumpLine: UInt32?

    func makeUIView(context: Context) -> UITextView {
        let view = UITextView()
        view.font = .monospacedSystemFont(ofSize: 15, weight: .regular)
        view.autocorrectionType = .no
        view.autocapitalizationType = .none
        view.spellCheckingType = .no
        view.smartQuotesType = .no
        view.smartDashesType = .no
        view.smartInsertDeleteType = .no
        view.keyboardDismissMode = .interactive
        view.alwaysBounceVertical = true
        view.textContainerInset = UIEdgeInsets(top: 10, left: 8, bottom: 10, right: 8)
        view.delegate = context.coordinator
        view.text = text
        return view
    }

    func updateUIView(_ view: UITextView, context: Context) {
        if view.text != text { view.text = text }
        if let line = jumpLine {
            DispatchQueue.main.async { jumpLine = nil }
            select(line: Int(line), in: view)
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(text: $text) }

    private func select(line: Int, in view: UITextView) {
        let lines = view.text.components(separatedBy: "\n")
        guard line >= 1, line <= lines.count else { return }
        let start = lines[..<(line - 1)].reduce(0) { $0 + ($1 as NSString).length + 1 }
        let range = NSRange(location: start, length: (lines[line - 1] as NSString).length)
        view.becomeFirstResponder()
        view.selectedRange = range
        view.scrollRangeToVisible(range)
    }

    final class Coordinator: NSObject, UITextViewDelegate {
        let text: Binding<String>
        init(text: Binding<String>) { self.text = text }
        func textViewDidChange(_ view: UITextView) { text.wrappedValue = view.text }
    }
}
