import SwiftUI
import AppKit
import UniformTypeIdentifiers

struct EditorActions {
    var copy: () -> Void
    var save: () -> Void
    var saveAs: () -> Void
    var share: (NSView) -> Void
    var copyText: () -> Void
    var autoRedact: () -> Void
    var dragFile: () -> URL?
}

struct EditorView: View {
    @ObservedObject var model: EditorModel
    let actions: EditorActions
    /// Height of the transparent title bar the top rows sit in.
    var barHeight: CGFloat = 52

    var body: some View {
        HStack(spacing: 0) {
            VStack(spacing: 0) {
                ToolBar(model: model, actions: actions)
                    .frame(height: barHeight)
                    .background(WindowDragArea())
                Divider()
                ZStack {
                    CanvasBackdrop()
                    CanvasView(model: model)
                    if let busy = model.busy {
                        HStack(spacing: 8) {
                            ProgressView().controlSize(.small)
                            Text(busy)
                        }
                        .padding(.horizontal, 14).padding(.vertical, 8)
                        .kireiGlass(in: Capsule())
                        .frame(maxHeight: .infinity, alignment: .top)
                        .padding(.top, 12)
                        .transition(.move(edge: .top).combined(with: .opacity))
                    }
                }
                .animation(.spring(duration: 0.3), value: model.busy)
                Divider()
                BottomBar(model: model, actions: actions)
            }
            Divider()
            Inspector(model: model, barHeight: barHeight)
                .frame(width: 284)
        }
        .ignoresSafeArea(.container, edges: .top)
    }
}

// MARK: - Top toolbar

private struct ToolBar: View {
    @ObservedObject var model: EditorModel
    let actions: EditorActions

    var body: some View {
        HStack(spacing: 10) {
            HStack(spacing: 2) {
                ForEach(Tool.allCases) { t in
                    Button { model.tool = t } label: {
                        Image(systemName: t.symbol)
                            .font(.system(size: 14, weight: .medium))
                            .frame(width: 30, height: 26)
                            .background(model.tool == t ? Color.accentColor.opacity(0.2) : .clear,
                                        in: RoundedRectangle(cornerRadius: 6))
                            .foregroundStyle(model.tool == t ? Color.accentColor : .primary)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help(t.title)
                }
            }

            Divider().frame(height: 20)

            HStack(spacing: 5) {
                ForEach(Palette.annotation, id: \.self) { c in
                    Button { model.applyColorToSelection(c) } label: {
                        Circle().fill(c.color)
                            .overlay(Circle().strokeBorder(.primary.opacity(0.25), lineWidth: 0.5))
                            .overlay(Circle().strokeBorder(Color.accentColor, lineWidth: 2)
                                .padding(-3).opacity(model.color == c ? 1 : 0))
                            .frame(width: 16, height: 16)
                    }
                    .buttonStyle(.plain)
                }
                CustomColorButton(current: model.color) { model.applyColorToSelection($0) }
                    .padding(.leading, 2)
            }

            Picker("", selection: Binding(get: { model.strokeLevel }, set: { model.applyStrokeToSelection($0) })) {
                Image(systemName: "line.diagonal").tag(0)
                Image(systemName: "line.diagonal").fontWeight(.bold).tag(1)
                Image(systemName: "line.diagonal").fontWeight(.black).tag(2)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 96)
            .help(L("Thickness / text size"))

            Spacer(minLength: 4)

            Button { model.undoManager?.undo() } label: { Image(systemName: "arrow.uturn.backward") }
                .help(L("Undo (⌘Z)"))
            Button { model.undoManager?.redo() } label: { Image(systemName: "arrow.uturn.forward") }
                .help(L("Redo (⇧⌘Z)"))

            Menu {
                Button(L("Copy Text (OCR)"), action: actions.copyText)
                Button(L("Hide Sensitive Data"), action: actions.autoRedact)
                Divider()
                Button(L("Remove All Annotations")) { model.clearAnnotations() }
                    .disabled(model.annotations.isEmpty)
                Button(L("Reset Crop")) { model.setCrop(nil) }
                    .disabled(model.manualCrop == nil)
            } label: {
                Image(systemName: "wand.and.stars")
            }
            .menuStyle(.borderlessButton)
            .frame(width: 36)
            .help(L("Smart tools"))
        }
        .padding(.leading, 84)
        .padding(.trailing, 12)
    }
}

/// Compact swatch that opens the system color panel. Replaces `ColorPicker`, whose
/// well is wider than its frame on recent macOS and draws over neighboring swatches.
private struct CustomColorButton: View {
    let current: RGBA
    let onChange: (RGBA) -> Void

    var body: some View {
        Button {
            ColorPanelBridge.shared.open(with: current.nsColor, onChange: onChange)
        } label: {
            Circle()
                .fill(AngularGradient(colors: [.red, .yellow, .green, .cyan, .blue, .purple, .red],
                                      center: .center))
                .overlay(Circle().strokeBorder(.primary.opacity(0.25), lineWidth: 0.5))
                .frame(width: 16, height: 16)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .help(L("Custom color"))
    }
}

private final class ColorPanelBridge: NSObject {
    static let shared = ColorPanelBridge()
    private var onChange: ((RGBA) -> Void)?

    func open(with color: NSColor, onChange: @escaping (RGBA) -> Void) {
        self.onChange = nil
        let panel = NSColorPanel.shared
        panel.showsAlpha = false
        panel.color = color
        self.onChange = onChange
        panel.setTarget(self)
        panel.setAction(#selector(colorChanged(_:)))
        panel.makeKeyAndOrderFront(nil)
    }

    @objc private func colorChanged(_ sender: NSColorPanel) {
        onChange?(RGBA(nsColor: sender.color))
    }
}

// MARK: - Bottom bar

private struct BottomBar: View {
    @ObservedObject var model: EditorModel
    let actions: EditorActions

    var body: some View {
        let size = model.layout.canvasSize
        HStack(spacing: 12) {
            DragThumbnail(model: model, dragFile: actions.dragFile)

            VStack(alignment: .leading, spacing: 1) {
                Text("\(Int(size.width)) × \(Int(size.height)) px")
                    .font(.callout.monospacedDigit().weight(.medium))
                Text(L("Drag the thumbnail to any app"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            ZoomControl(model: model)
                .padding(.leading, 4)

            Spacer()

            ShareButton(action: actions.share)
                .frame(width: 34, height: 24)
            Menu {
                Button(L("Save As…"), action: actions.saveAs)
            } label: {
                Label(L("Save"), systemImage: "square.and.arrow.down")
            } primaryAction: {
                actions.save()
            }
            .fixedSize()
            .help(L("Save to the default folder (⌘S)"))
            Button(action: actions.copy) {
                Label(L("Copy"), systemImage: "doc.on.doc")
                    .padding(.horizontal, 6)
            }
            .buttonStyle(.borderedProminent)
            .help(L("Copy image (⌘C)"))
        }
        .controlSize(.large)
        .padding(.horizontal, 12)
        .frame(height: 56)
    }
}

/// Live miniature of the final image; dragging it drops the exported PNG into other apps.
private struct DragThumbnail: View {
    @ObservedObject var model: EditorModel
    let dragFile: () -> URL?
    @State private var image: CGImage?

    private struct Key: Equatable {
        var style: Style
        var crop: CGRect
        var edge: RGBA
    }

    var body: some View {
        ZStack {
            if let image {
                Image(decorative: image, scale: 2)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
            } else {
                Color.primary.opacity(0.06)
            }
        }
        .frame(width: 64, height: 40)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.primary.opacity(0.12), lineWidth: 0.5))
        .shadow(color: .black.opacity(0.15), radius: 3, y: 1)
        .contentShape(Rectangle())
        .onDrag {
            guard let url = dragFile() else { return NSItemProvider() }
            return NSItemProvider(contentsOf: url) ?? NSItemProvider()
        }
        .help(L("Drag the final image to Finder, Slack, email…"))
        .task(id: Key(style: model.style, crop: model.crop, edge: model.edgeColor)) {
            let src = model.source, style = model.style, crop = model.crop, edge = model.edgeColor
            let layout = CanvasLayout(style: style, crop: crop)
            let scale = 128 / max(layout.canvasSize.width, layout.canvasSize.height)
            let img = await Task.detached(priority: .utility) {
                Renderer.render(source: src, style: style, crop: crop, edgeColor: edge,
                                annotations: [], effects: nil, scale: scale)
            }.value
            image = img
        }
    }
}

private struct ZoomControl: View {
    @ObservedObject var model: EditorModel

    var body: some View {
        HStack(spacing: 2) {
            Button { model.zoomOut() } label: { Image(systemName: "minus.magnifyingglass") }
                .help(L("Zoom Out") + " (⌘−)")
            Menu {
                Button(L("Fit in Window")) { model.zoomToFit() }
                Divider()
                ForEach(EditorModel.zoomSteps, id: \.self) { z in
                    Button("\(Int((z * 100).rounded()))%") { model.zoom = z }
                }
            } label: {
                Text(model.zoom == nil ? L("Fit") : "\(Int((model.effectiveZoom * 100).rounded()))%")
                    .monospacedDigit()
                    .frame(minWidth: 52)
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .help(L("100% shows each captured pixel as one screen pixel (⌘0)"))
            Button { model.zoomIn() } label: { Image(systemName: "plus.magnifyingglass") }
                .help(L("Zoom In") + " (⌘+)")
        }
        .buttonStyle(.borderless)
    }
}

/// NSButton wrapper so the sharing picker can anchor to a real view.
struct ShareButton: NSViewRepresentable {
    let action: (NSView) -> Void

    func makeNSView(context: Context) -> NSButton {
        let b = NSButton(image: NSImage(systemSymbolName: "square.and.arrow.up", accessibilityDescription: L("Share"))!,
                         target: context.coordinator, action: #selector(Coordinator.click(_:)))
        b.bezelStyle = .push
        b.toolTip = L("Share")
        return b
    }

    func updateNSView(_ v: NSButton, context: Context) { context.coordinator.action = action }
    func makeCoordinator() -> Coordinator { Coordinator(action: action) }

    final class Coordinator: NSObject {
        var action: (NSView) -> Void
        init(action: @escaping (NSView) -> Void) { self.action = action }
        @objc func click(_ sender: NSButton) { action(sender) }
    }
}

// MARK: - Inspector

private struct Inspector: View {
    @ObservedObject var model: EditorModel
    var barHeight: CGFloat

    var body: some View {
        VStack(spacing: 0) {
            InspectorHeader(model: model)
                .frame(height: barHeight)
                .background(WindowDragArea())
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    section(L("Styles")) { StyleGallery(model: model) }
                    Divider()
                    section(L("Background")) { BackgroundPicker(style: $model.style) }
                    Divider()
                    section(L("Frame")) { FrameControls(style: $model.style) }
                    Divider()
                    section(L("Layout")) { LayoutControls(style: $model.style) }
                }
                .padding(14)
            }
        }
        .background(VisualEffect(material: .sidebar))
    }

    private func section<C: View>(_ title: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title.uppercased())
                .font(.caption.weight(.semibold))
                .tracking(0.6)
                .foregroundStyle(.secondary)
            content()
        }
    }
}

private struct InspectorHeader: View {
    @ObservedObject var model: EditorModel
    @State private var showingShortcuts = false

    var body: some View {
        HStack(spacing: 6) {
            Text(L("Style")).font(.headline)
            Spacer()
            Button { model.style = .default } label: {
                Image(systemName: "arrow.counterclockwise").frame(width: 24, height: 24)
            }
            .help(L("Restore Default"))
            Button { showingShortcuts.toggle() } label: { Image(systemName: "keyboard").frame(width: 24, height: 24) }
                .help(L("Shortcuts"))
                .popover(isPresented: $showingShortcuts) {
                    Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 6) {
                        shortcut("⌘C", L("Copy image"))
                        shortcut("⌘S", L("Save"))
                        shortcut("⌘Z", L("Undo"))
                        shortcut("⌫", L("Delete annotation"))
                        shortcut(L("⇧ click"), L("Add to selection"))
                        shortcut("⌘A", L("Select All"))
                        shortcut("⌘D", L("Duplicate"))
                        shortcut(L("⌥ drag"), L("Duplicate while moving"))
                        shortcut(L("Double-click"), L("Edit text or number"))
                        shortcut("← → ↑ ↓", L("Nudge (⇧ = 10 px)"))
                        shortcut("V A R O H T N P B C", L("Tools"))
                    }
                    .font(.callout)
                    .padding(14)
                }
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 14)
    }

    private func shortcut(_ k: String, _ label: String) -> some View {
        GridRow {
            Text(k).monospaced().foregroundStyle(.secondary)
            Text(label)
        }
    }
}

/// Saved styles as visual cards; the first card saves the current style.
private struct StyleGallery: View {
    @ObservedObject var model: EditorModel
    @ObservedObject var presets = PresetStore.shared
    @State private var newPresetName = ""
    @State private var showingSave = false

    private let columns = Array(repeating: GridItem(.flexible(), spacing: 8), count: 3)

    var body: some View {
        LazyVGrid(columns: columns, spacing: 10) {
            Button {
                newPresetName = String(format: L("Style %d"), presets.presets.count + 1)
                showingSave = true
            } label: {
                VStack(spacing: 4) {
                    RoundedRectangle(cornerRadius: 7)
                        .strokeBorder(.secondary.opacity(0.5), style: StrokeStyle(lineWidth: 1, dash: [4, 3]))
                        .overlay(Image(systemName: "plus").foregroundStyle(.secondary))
                        .aspectRatio(1.4, contentMode: .fit)
                    Text(L("Save")).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(L("Save the current style"))
            .popover(isPresented: $showingSave) { savePopover }

            ForEach(presets.presets) { p in
                Button { withAnimation(.easeOut(duration: 0.2)) { model.style = p.style } } label: {
                    VStack(spacing: 4) {
                        card(p.style, selected: p.style == model.style)
                        Text(p.name).font(.caption2).lineLimit(1).truncationMode(.tail)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(p.name)
                .contextMenu {
                    Button(L("Apply Style")) { model.style = p.style }
                    Button(L("Update with Current Style")) { presets.update(p, style: model.style) }
                    Divider()
                    Button(L("Delete"), role: .destructive) { presets.remove(p) }
                }
            }
        }
    }

    private func card(_ style: Style, selected: Bool) -> some View {
        Color.clear
            .aspectRatio(1.4, contentMode: .fit)
            .overlay {
                if let img = StyleThumbnails.style(style) {
                    Image(decorative: img, scale: 2).resizable().aspectRatio(contentMode: .fill)
                } else {
                    Checkerboard()
                }
            }
            .clipShape(RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(.primary.opacity(0.12), lineWidth: 0.5))
            .overlay(RoundedRectangle(cornerRadius: 9).strokeBorder(Color.accentColor, lineWidth: 2)
                .padding(-3).opacity(selected ? 1 : 0))
    }

    private var savePopover: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(L("Save Style")).font(.headline)
            TextField(L("Name"), text: $newPresetName)
                .frame(width: 200)
                .onSubmit(save)
            HStack {
                Spacer()
                Button(L("Cancel")) { showingSave = false }
                Button(L("Save"), action: save).keyboardShortcut(.defaultAction)
            }
        }
        .padding()
    }

    private func save() {
        let name = newPresetName.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }
        presets.add(name: name, style: model.style)
        showingSave = false
    }
}

// MARK: Frame

private struct FrameControls: View {
    @Binding var style: Style

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 6) {
                ForEach(WindowFrame.allCases) { f in
                    Button { withAnimation(.easeOut(duration: 0.2)) { style.frame = f } } label: {
                        FrameGlyph(frame: f)
                            .frame(maxWidth: .infinity)
                            .frame(height: 34)
                            .background(style.frame == f ? Color.accentColor.opacity(0.18) : Color.primary.opacity(0.05),
                                        in: RoundedRectangle(cornerRadius: 7))
                            .overlay(RoundedRectangle(cornerRadius: 7)
                                .strokeBorder(style.frame == f ? Color.accentColor : .clear, lineWidth: 1.5))
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help(f.title)
                }
            }
            InspectorRow(L("Signature")) {
                TextField(L("@yourname"), text: $style.watermark)
                    .textFieldStyle(.roundedBorder)
                    .controlSize(.small)
            }
            .help(L("Small text in the corner of the image"))
        }
    }
}

private struct FrameGlyph: View {
    let frame: WindowFrame

    var body: some View {
        let bar: Color = frame.isDark ? Color(white: 0.25) : Color(white: 0.88)
        let body: Color = frame.isDark ? Color(white: 0.15) : .white
        if frame == .none {
            Image(systemName: "rectangle.slash").foregroundStyle(.secondary)
        } else {
            VStack(spacing: 0) {
                HStack(spacing: 1.5) {
                    ForEach(0..<3) { i in
                        Circle().fill([Color.red, .yellow, .green][i]).frame(width: 3, height: 3)
                    }
                    if frame.isBrowser {
                        Capsule().fill(body.opacity(0.9)).frame(width: 10, height: 3)
                            .padding(.leading, 1)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 3)
                .frame(height: 7)
                .background(bar)
                body
            }
            .frame(width: 30, height: 22)
            .clipShape(RoundedRectangle(cornerRadius: 3))
            .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(.primary.opacity(0.2), lineWidth: 0.5))
        }
    }
}

// MARK: Layout

private struct LayoutControls: View {
    @Binding var style: Style

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            InspectorRow(L("Aspect Ratio")) {
                Picker("", selection: $style.aspect) {
                    ForEach(AspectRatio.allCases) { Text($0.label).tag($0) }
                }
                .labelsHidden()
            }
            InspectorRow(L("Position")) {
                AlignmentGrid(alignment: $style.alignment)
                Spacer(minLength: 0)
            }

            Toggle(isOn: $style.autoBalance) {
                Text(L("Auto-balance")).frame(maxWidth: .infinity, alignment: .leading)
            }
            .toggleStyle(.switch)
            .controlSize(.mini)
            .font(.callout)
            .help(L("Removes empty borders and centers the content"))
            .padding(.top, 4)

            SliderRow(L("Padding"), value: $style.padding, range: 0...0.3)
            SliderRow(L("Inset"), value: $style.inset, range: 0...0.08)
            SliderRow(L("Corners"), value: $style.cornerRadius, range: 0...0.05)
            SliderRow(L("Shadow"), value: $style.shadow, range: 0...1)
        }
    }
}

/// Label on the left, control on the right; every row shares the same label width.
private struct InspectorRow<Content: View>: View {
    let title: String
    let content: Content

    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        HStack(spacing: 8) {
            Text(title)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .frame(width: 76, alignment: .leading)
            content
        }
        .font(.callout)
    }
}

private struct SliderRow: View {
    let title: String
    @Binding var value: Double
    let range: ClosedRange<Double>
    /// Shows the raw value with this suffix instead of a 0–100 scale.
    var unit: String?

    init(_ title: String, value: Binding<Double>, range: ClosedRange<Double>, unit: String? = nil) {
        self.title = title
        _value = value
        self.range = range
        self.unit = unit
    }

    var body: some View {
        InspectorRow(title) {
            Slider(value: $value, in: range)
                .controlSize(.small)
            Text(display)
                .monospacedDigit()
                .foregroundStyle(.secondary)
                .frame(width: 30, alignment: .trailing)
        }
    }

    private var display: String {
        if let unit { return "\(Int(value.rounded()))\(unit)" }
        return "\(Int(((value - range.lowerBound) / (range.upperBound - range.lowerBound) * 100).rounded()))"
    }
}

private struct AlignmentGrid: View {
    @Binding var alignment: Int

    var body: some View {
        Grid(horizontalSpacing: 3, verticalSpacing: 3) {
            ForEach(0..<3) { row in
                GridRow {
                    ForEach(0..<3) { col in
                        let i = row * 3 + col
                        Button { alignment = i } label: {
                            RoundedRectangle(cornerRadius: 3)
                                .fill(alignment == i ? Color.accentColor : Color.primary.opacity(0.1))
                                .frame(width: 22, height: 14)
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .help(L("Where the capture sits on the background"))
    }
}

// MARK: Background

private struct BackgroundPicker: View {
    @Binding var style: Style
    @State private var kind: Kind = .gradient

    private enum Kind: CaseIterable {
        case gradient, mesh, solid, image

        init(_ bg: Background) {
            switch bg {
            case .gradient: self = .gradient
            case .mesh: self = .mesh
            case .image: self = .image
            case .none, .solid: self = .solid
            }
        }

        var title: String {
            switch self {
            case .gradient: return L("Gradient")
            case .mesh: return L("Mesh")
            case .solid: return L("Color")
            case .image: return L("Image")
            }
        }
    }

    private let columns = Array(repeating: GridItem(.flexible(), spacing: 6), count: 8)

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Custom tabs: the native segmented picker doesn't stretch to the column width.
            HStack(spacing: 2) {
                ForEach(Kind.allCases, id: \.self) { k in
                    Button { withAnimation(.easeOut(duration: 0.15)) { kind = k } } label: {
                        Text(k.title)
                            .font(.callout)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 4)
                            .background(kind == k ? Color.primary.opacity(0.12) : .clear,
                                        in: RoundedRectangle(cornerRadius: 5))
                            .foregroundStyle(kind == k ? .primary : .secondary)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(2)
            .background(Color.primary.opacity(0.05), in: RoundedRectangle(cornerRadius: 7))

            switch kind {
            case .gradient:
                LazyVGrid(columns: columns, spacing: 6) {
                    ForEach(Array(Gradients.presets.enumerated()), id: \.offset) { _, colors in
                        swatch(.gradient(colors, angle: 135)) {
                            LinearGradient(colors: colors.map(\.color), startPoint: .topLeading, endPoint: .bottomTrailing)
                        }
                    }
                }
                if case .gradient(let colors, let angle) = style.background {
                    SliderRow(L("Angle"),
                              value: Binding(get: { angle }, set: { style.background = .gradient(colors, angle: $0) }),
                              range: 0...360, unit: "°")
                }
            case .mesh:
                LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 6), count: 4), spacing: 6) {
                    ForEach(Array(Gradients.meshes.enumerated()), id: \.offset) { _, colors in
                        swatch(.mesh(colors)) {
                            if let img = StyleThumbnails.background(.mesh(colors)) {
                                Image(decorative: img, scale: 1).resizable()
                            }
                        }
                    }
                }
            case .solid:
                LazyVGrid(columns: columns, spacing: 6) {
                    swatch(.none) {
                        ZStack {
                            Checkerboard()
                            Image(systemName: "nosign").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    .help(L("Transparent"))
                    ForEach(Gradients.solids, id: \.self) { c in
                        swatch(.solid(c)) { c.color }
                    }
                    customColorSwatch
                }
            case .image:
                HStack(spacing: 8) {
                    Button(action: pickImage) {
                        Label(L("Image…"), systemImage: "photo").frame(maxWidth: .infinity)
                    }
                    Button(action: useWallpaper) {
                        Label(L("Wallpaper"), systemImage: "menubar.dock.rectangle").frame(maxWidth: .infinity)
                    }
                    .help(L("Use the current wallpaper"))
                }
                if case .image(let path) = style.background {
                    Text(URL(fileURLWithPath: path).lastPathComponent)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    SliderRow(L("Blur"), value: $style.backgroundBlur, range: 0...1)
                        .help(L("Blurs the background image"))
                }
            }
            if style.background != .none {
                SliderRow(L("Grain"), value: $style.grain, range: 0...1)
                    .help(L("Adds a subtle film texture to the background"))
            }
        }
        .onAppear { kind = Kind(style.background) }
        .onChange(of: style.background) { kind = Kind(style.background) }
    }

    /// Opens the system color panel; selected when the background is a color outside the presets.
    private var customColorSwatch: some View {
        var current: RGBA? { if case .solid(let c) = style.background { return c }; return nil }
        let isCustom = current.map { !Gradients.solids.contains($0) } ?? false
        return Button {
            ColorPanelBridge.shared.open(with: (current ?? .white).nsColor) { style.background = .solid($0) }
        } label: {
            swatchShape(selected: isCustom) {
                AngularGradient(colors: [.red, .yellow, .green, .cyan, .blue, .purple, .red], center: .center)
            }
        }
        .buttonStyle(.plain)
        .help(L("Custom color"))
    }

    private func isSelected(_ bg: Background) -> Bool {
        switch (bg, style.background) {
        case (.gradient(let a, _), .gradient(let b, _)): return a == b
        default: return bg == style.background
        }
    }

    private func swatch<C: View>(_ bg: Background, @ViewBuilder _ content: () -> C) -> some View {
        Button {
            if case .gradient(let c, _) = bg, case .gradient(_, let angle) = style.background {
                style.background = .gradient(c, angle: angle)
            } else {
                style.background = bg
            }
        } label: {
            swatchShape(selected: isSelected(bg), content)
        }
        .buttonStyle(.plain)
    }

    private func swatchShape<C: View>(selected: Bool, @ViewBuilder _ content: () -> C) -> some View {
        Color.clear
            .aspectRatio(1, contentMode: .fit)
            .overlay(content())
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.primary.opacity(0.12), lineWidth: 0.5))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Color.accentColor, lineWidth: 2)
                .padding(-3).opacity(selected ? 1 : 0))
            .contentShape(Rectangle())
    }

    private func pickImage() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.image]
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            style.background = .image(path: url.path)
        }
    }

    private func useWallpaper() {
        guard let screen = NSScreen.main, let url = NSWorkspace.shared.desktopImageURL(for: screen) else { return }
        var isDir: ObjCBool = false
        if FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir), !isDir.boolValue,
           ImageCache.shared.image(path: url.path) != nil {
            style.background = .image(path: url.path)
        } else {
            HUD.show(L("Couldn't read the wallpaper"), symbol: "exclamationmark.triangle.fill")
        }
    }
}
