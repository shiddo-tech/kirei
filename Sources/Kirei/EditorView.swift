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
    var pin: () -> Void
    var dragFile: () -> URL?
}

struct EditorView: View {
    @ObservedObject var model: EditorModel
    let actions: EditorActions
    /// Height of the transparent title bar the top rows sit in.
    var barHeight: CGFloat = 52
    @AppStorage(EditorView.inspectorKey) private var showsInspector = true

    static let inspectorKey = "inspector.visible"

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
            if showsInspector {
                HStack(spacing: 0) {
                    Divider()
                    Inspector(model: model, barHeight: barHeight)
                        .frame(width: 284)
                }
                .transition(.move(edge: .trailing))
            }
        }
        .animation(.easeOut(duration: 0.2), value: showsInspector)
        .ignoresSafeArea(.container, edges: .top)
    }
}

// MARK: - Top toolbar

/// Similar tools share one toolbar slot; clicking the active slot again switches between them.
private enum ToolGroup: CaseIterable {
    case select, arrow, shape, highlight, text, counter, redact, crop

    var tools: [Tool] {
        switch self {
        case .select: return [.select]
        case .arrow: return [.arrow]
        case .shape: return [.rect, .ellipse]
        case .highlight: return [.highlight]
        case .text: return [.text]
        case .counter: return [.counter]
        case .redact: return [.pixelate, .blur]
        case .crop: return [.crop]
        }
    }

    static func containing(_ tool: Tool) -> ToolGroup? {
        allCases.first { $0.tools.contains(tool) }
    }
}

private extension Tool {
    var usesColor: Bool {
        switch self {
        case .arrow, .rect, .ellipse, .highlight, .text, .counter: return true
        case .select, .pixelate, .blur, .crop: return false
        }
    }
}

private struct ToolBar: View {
    @ObservedObject var model: EditorModel
    let actions: EditorActions
    @State private var lastInGroup: [ToolGroup: Tool] = [:]
    @AppStorage(EditorView.inspectorKey) private var showsInspector = true

    private var showsStyle: Bool { model.tool.usesColor || !model.selection.isEmpty }

    var body: some View {
        HStack(spacing: 10) {
            HStack(spacing: 2) {
                ForEach(ToolGroup.allCases, id: \.self) { g in
                    ToolGroupButton(group: g,
                                    current: g.tools.contains(model.tool) ? model.tool : (lastInGroup[g] ?? g.tools[0]),
                                    active: g.tools.contains(model.tool)) { model.tool = $0 }
                }
            }

            if showsStyle {
                AnnotationStyleButton(model: model)
                    .transition(.opacity.combined(with: .scale(scale: 0.8)))
            }

            Spacer(minLength: 4)

            Menu {
                Button(L("Copy Text (OCR)"), action: actions.copyText)
                Button(L("Hide Sensitive Data"), action: actions.autoRedact)
                Button(L("Pin to Screen"), action: actions.pin)
                Divider()
                Button(L("Remove All Annotations")) { model.clearAnnotations() }
                    .disabled(model.annotations.isEmpty)
                Button(L("Reset Crop")) { model.setCrop(nil) }
                    .disabled(model.manualCrop == nil)
            } label: {
                Image(systemName: "wand.and.stars")
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .help(L("Smart tools"))

            MoreMenu(model: model)

            Button { showsInspector.toggle() } label: {
                Image(systemName: "sidebar.right")
            }
            .buttonStyle(.borderless)
            .help(L(showsInspector ? "Hide Inspector" : "Show Inspector") + " (⌥⌘I)")
        }
        .padding(.leading, 84)
        .padding(.trailing, 14)
        .animation(.easeOut(duration: 0.15), value: showsStyle)
        .onChange(of: model.tool) {
            if let g = ToolGroup.containing(model.tool) { lastInGroup[g] = model.tool }
        }
    }
}

private struct ToolGroupButton: View {
    let group: ToolGroup
    let current: Tool
    let active: Bool
    let select: (Tool) -> Void

    private var isGroup: Bool { group.tools.count > 1 }

    var body: some View {
        Button {
            if active, isGroup, let i = group.tools.firstIndex(of: current) {
                select(group.tools[(i + 1) % group.tools.count])
            } else {
                select(current)
            }
        } label: {
            Image(systemName: current.symbol)
                .font(.system(size: 14, weight: .medium))
                .frame(width: 30, height: 26)
                .overlay(alignment: .bottomTrailing) {
                    if isGroup {
                        CornerMark()
                            .fill(.secondary)
                            .frame(width: 4, height: 4)
                            .padding(3)
                    }
                }
                .background(active ? Color.primary.opacity(0.1) : .clear, in: RoundedRectangle(cornerRadius: 6))
                .foregroundStyle(active ? .primary : .secondary)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(isGroup ? "\(current.title) · \(L("Click again to switch"))" : current.title)
        .contextMenu {
            if isGroup {
                ForEach(group.tools) { t in Button(t.title) { select(t) } }
            }
        }
    }
}

private struct CornerMark: Shape {
    func path(in r: CGRect) -> Path {
        Path { p in
            p.move(to: CGPoint(x: r.maxX, y: r.minY))
            p.addLine(to: CGPoint(x: r.maxX, y: r.maxY))
            p.addLine(to: CGPoint(x: r.minX, y: r.maxY))
            p.closeSubpath()
        }
    }
}

private struct AnnotationStyleButton: View {
    @ObservedObject var model: EditorModel
    @State private var open = false

    var body: some View {
        Button { open.toggle() } label: {
            Circle().fill(model.color.color)
                .overlay(Circle().strokeBorder(.primary.opacity(0.25), lineWidth: 0.5))
                .frame(width: 16, height: 16)
                .padding(5)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(L("Color and thickness"))
        .popover(isPresented: $open, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 8) {
                    ForEach(Palette.annotation, id: \.self) { c in
                        Button { model.applyColorToSelection(c) } label: {
                            Circle().fill(c.color)
                                .overlay(Circle().strokeBorder(.primary.opacity(0.25), lineWidth: 0.5))
                                .overlay(Circle().strokeBorder(Color.accentColor, lineWidth: 2)
                                    .padding(-3).opacity(model.color == c ? 1 : 0))
                                .frame(width: 20, height: 20)
                        }
                        .buttonStyle(.plain)
                    }
                    CustomColorButton(current: model.color, size: 20) { model.applyColorToSelection($0) }
                }
                Picker("", selection: Binding(get: { model.strokeLevel }, set: { model.applyStrokeToSelection($0) })) {
                    Image(systemName: "line.diagonal").tag(0)
                    Image(systemName: "line.diagonal").fontWeight(.bold).tag(1)
                    Image(systemName: "line.diagonal").fontWeight(.black).tag(2)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .help(L("Thickness / text size"))
            }
            .padding(16)
        }
    }
}

/// Compact swatch that opens the system color panel. Replaces `ColorPicker`, whose
/// well is wider than its frame on recent macOS and draws over neighboring swatches.
private struct CustomColorButton: View {
    let current: RGBA
    var size: CGFloat = 16
    let onChange: (RGBA) -> Void

    var body: some View {
        Button {
            ColorPanelBridge.shared.open(with: current.nsColor, onChange: onChange)
        } label: {
            Circle()
                .fill(AngularGradient(colors: [.red, .yellow, .green, .cyan, .blue, .purple, .red],
                                      center: .center))
                .overlay(Circle().strokeBorder(.primary.opacity(0.25), lineWidth: 0.5))
                .frame(width: size, height: size)
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
        HStack(spacing: 14) {
            DragThumbnail(model: model, dragFile: actions.dragFile)

            HStack(spacing: 10) {
                Text("\(Int(size.width)) × \(Int(size.height))")
                    .font(.system(size: 11, weight: .medium).monospacedDigit())
                    .foregroundStyle(.secondary)
                BarHairline()
                ZoomControl(model: model)
            }
            .padding(.horizontal, 12)
            .frame(height: 26)
            .background(Capsule().fill(.primary.opacity(0.045)))

            Spacer()

            HStack(spacing: 0) {
                ShareButton(action: actions.share)
                    .frame(width: 32, height: 28)
                BarHairline()
                Menu {
                    Button(L("Save As…"), action: actions.saveAs)
                } label: {
                    Image(systemName: "square.and.arrow.down")
                } primaryAction: {
                    actions.save()
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
                .frame(width: 32, height: 28)
                .help(L("Save to the default folder (⌘S)"))
            }
            .padding(.horizontal, 2)
            .background(Capsule().fill(.primary.opacity(0.045)))
            .foregroundStyle(.secondary)

            Button(action: actions.copy) {
                HStack(spacing: 6) {
                    Text(L("Copy"))
                    Text("⌘C")
                        .font(.system(size: 10, weight: .medium, design: .rounded))
                        .opacity(0.6)
                }
            }
            .buttonStyle(CopyPillStyle())
            .help(L("Copy image (⌘C)"))
        }
        .padding(.horizontal, 16)
        .frame(height: 56)
        .background(alignment: .top) {
            LinearGradient(colors: [.primary.opacity(0.025), .clear], startPoint: .top, endPoint: .bottom)
        }
    }
}

private struct BarHairline: View {
    var body: some View {
        Rectangle()
            .fill(.primary.opacity(0.12))
            .frame(width: 0.5, height: 12)
    }
}

private struct CopyPillStyle: ButtonStyle {
    @State private var hovering = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(Color.accentColor)
            .padding(.horizontal, 12)
            .frame(height: 28)
            .background {
                Capsule()
                    .fill(Color.accentColor.opacity(configuration.isPressed ? 0.2 : hovering ? 0.16 : 0.12))
            }
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.15), value: hovering)
            .animation(.easeOut(duration: 0.1), value: configuration.isPressed)
            .onHover { hovering = $0 }
            .contentShape(Capsule())
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
        Menu {
            Button(L("Zoom In") + " (⌘+)") { model.zoomIn() }
            Button(L("Zoom Out") + " (⌘−)") { model.zoomOut() }
            Button(L("Fit in Window")) { model.zoomToFit() }
            Divider()
            ForEach(EditorModel.zoomSteps, id: \.self) { z in
                Button("\(Int((z * 100).rounded()))%") { model.zoom = z }
            }
        } label: {
            Text(model.zoom == nil ? L("Fit") : "\(Int((model.effectiveZoom * 100).rounded()))%")
                .font(.caption.monospacedDigit())
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .foregroundStyle(.secondary)
        .help(L("100% shows each captured pixel as one screen pixel (⌘0)"))
    }
}

/// NSButton wrapper so the sharing picker can anchor to a real view.
struct ShareButton: NSViewRepresentable {
    let action: (NSView) -> Void

    func makeNSView(context: Context) -> NSButton {
        let b = NSButton(image: NSImage(systemSymbolName: "square.and.arrow.up", accessibilityDescription: L("Share"))!,
                         target: context.coordinator, action: #selector(Coordinator.click(_:)))
        b.isBordered = false
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
            Color.clear
                .frame(height: barHeight)
                .background(WindowDragArea())
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    section(L("Styles")) { StyleGallery(model: model) }
                    section(L("Background")) { BackgroundPicker(style: $model.style) }
                    VStack(alignment: .leading, spacing: 16) {
                        CollapsibleSection(L("Frame"), key: "frame", summary: model.style.frame.title) {
                            FrameControls(style: $model.style)
                        }
                        CollapsibleSection(L("Layout"), key: "layout",
                                           summary: model.style.aspect.label.components(separatedBy: " · ")[0]) {
                            LayoutControls(style: $model.style)
                        }
                        CollapsibleSection(L("Advanced"), key: "advanced") {
                            AdvancedControls(style: $model.style)
                        }
                    }
                }
                .padding(16)
            }
        }
        .background(VisualEffect(material: .sidebar))
    }

    private func section<C: View>(_ title: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.callout.weight(.semibold))
            content()
        }
    }
}

/// Open/closed state is remembered between sessions.
private struct CollapsibleSection<Content: View>: View {
    let title: String
    let summary: String?
    let content: Content
    @AppStorage private var expanded: Bool

    init(_ title: String, key: String, summary: String? = nil, @ViewBuilder content: () -> Content) {
        self.title = title
        self.summary = summary
        self.content = content()
        _expanded = AppStorage(wrappedValue: false, "inspector.expanded.\(key)")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Button { withAnimation(.easeOut(duration: 0.2)) { expanded.toggle() } } label: {
                HStack(spacing: 6) {
                    Text(title).font(.callout.weight(.semibold))
                    Spacer(minLength: 8)
                    if !expanded, let summary {
                        Text(summary)
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    Image(systemName: "chevron.right")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.tertiary)
                        .rotationEffect(.degrees(expanded ? 90 : 0))
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if expanded {
                content.transition(.opacity)
            }
        }
    }
}

private struct MoreMenu: View {
    @ObservedObject var model: EditorModel
    @State private var showingShortcuts = false

    var body: some View {
        Menu {
            Button(L("Restore Default")) { model.style = .default }
            Button(L("Shortcuts") + "…") { showingShortcuts = true }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fixedSize()
        .help(L("More options"))
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
                shortcut("⌥⌘I", L("Show or hide inspector"))
            }
            .font(.callout)
            .padding(14)
        }
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
            SliderRow(L("Corners"), value: $style.cornerRadius, range: 0...0.05)
            SliderRow(L("Shadow"), value: $style.shadow, range: 0...1)
        }
    }
}

private struct AdvancedControls: View {
    @Binding var style: Style

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            InspectorRow(L("Signature")) {
                TextField(L("@yourname"), text: $style.watermark)
                    .textFieldStyle(.roundedBorder)
                    .controlSize(.small)
            }
            .help(L("Small text in the corner of the image"))

            SliderRow(L("Inset"), value: $style.inset, range: 0...0.08)
            if case .gradient(let colors, let angle) = style.background {
                SliderRow(L("Angle"),
                          value: Binding(get: { angle }, set: { style.background = .gradient(colors, angle: $0) }),
                          range: 0...360, unit: "°")
            }
            if style.background != .none {
                SliderRow(L("Grain"), value: $style.grain, range: 0...1)
                    .help(L("Adds a subtle film texture to the background"))
            }

            Toggle(isOn: $style.hairline) {
                Text(L("Fine edge")).frame(maxWidth: .infinity, alignment: .leading)
            }
            .toggleStyle(.switch)
            .controlSize(.mini)
            .font(.callout)
            .help(L("A thin line that separates the screenshot from the background"))
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
    @State private var kind: Kind = .solid
    @State private var showAll = false

    private enum Kind: CaseIterable {
        case art, solid, image

        init(_ bg: Background) {
            switch bg {
            case .image: self = .image
            case .art, .gradient, .mesh: self = .art
            case .none, .solid: self = .solid
            }
        }

        var title: String {
            switch self {
            case .art: return L("Art")
            case .solid: return L("Color")
            case .image: return L("Image")
            }
        }
    }

    private let columns = Array(repeating: GridItem(.flexible(), spacing: 6), count: 8)
    private let artColumns = Array(repeating: GridItem(.flexible(), spacing: 6), count: 6)

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
            case .art:
                LazyVGrid(columns: artColumns, spacing: 6) {
                    ForEach(Array(visible(Art.presets, row: 12).enumerated()), id: \.offset) { _, art in
                        artSwatch(art)
                    }
                }
                HStack {
                    moreButton(total: Art.presets.count, row: 12)
                    Spacer()
                    if case .art(let current) = style.background {
                        Button { style.background = .art(current.reseeded()) } label: {
                            Label(L("Vary"), systemImage: "shuffle").font(.caption)
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(.secondary)
                        .help(L("Keeps the colors and draws new shapes"))
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
                    ForEach(visible(Gradients.solids, row: 6), id: \.self) { c in
                        swatch(.solid(c)) { c.color }
                    }
                    customColorSwatch
                }
                moreButton(total: Gradients.solids.count, row: 6)
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
                if case .image = style.background {
                    SliderRow(L("Blur"), value: $style.backgroundBlur, range: 0...1)
                        .help(L("Blurs the background image"))
                }
            }
        }
        .onAppear { sync() }
        .onChange(of: style.background) { sync() }
    }

    private func sync() {
        kind = Kind(style.background)
        if !showAll, selectionIsHidden { showAll = true }
    }

    private var selectionIsHidden: Bool {
        switch style.background {
        case .solid(let c): return !Gradients.solids.prefix(6).contains(c) && Gradients.solids.contains(c)
        case .art(let a): return !Art.presets.prefix(12).contains { a.isVariant(of: $0) }
        default: return false
        }
    }

    private func visible<T>(_ items: [T], row: Int) -> [T] {
        showAll ? items : Array(items.prefix(row))
    }

    @ViewBuilder
    private func moreButton(total: Int, row: Int) -> some View {
        if total > row {
            Button { withAnimation(.easeOut(duration: 0.2)) { showAll.toggle() } } label: {
                Text(showAll ? L("Show Less") : L("Show More"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
        }
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

    private func artSwatch(_ art: Art) -> some View {
        var selected: Bool { if case .art(let a) = style.background { return a.isVariant(of: art) }; return false }
        return Button { if !selected { style.background = .art(art) } } label: {
            swatchShape(selected: selected) { ArtThumbnail(art: art) }
        }
        .buttonStyle(.plain)
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
            // The image layer's clip isn't antialiased at fractional swatch widths; flattening fixes the jagged corners.
            .drawingGroup()
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
        guard let screen = NSScreen.main else { return }
        var isDir: ObjCBool = false
        if let url = NSWorkspace.shared.desktopImageURL(for: screen),
           FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir), !isDir.boolValue,
           ImageCache.shared.image(path: url.path) != nil {
            style.background = .image(path: url.path)
            return
        }
        CaptureService.captureWallpaper(of: screen) { image in
            if let image, let path = CaptureService.saveWallpaper(image) {
                style.background = .image(path: path)
            } else {
                HUD.show(L("Couldn't read the wallpaper"), symbol: "exclamationmark.triangle.fill")
            }
        }
    }
}


private struct ArtThumbnail: View {
    let art: Art
    @State private var image: CGImage?

    var body: some View {
        ZStack {
            Color(.sRGB, red: art.palette[0].r, green: art.palette[0].g, blue: art.palette[0].b)
            if let image {
                Image(decorative: image, scale: 1).resizable().scaledToFill()
            }
        }
        .task(id: art) {
            let art = art
            image = await Task.detached(priority: .userInitiated) {
                ArtCache.thumbnails.image(art, for: CGSize(width: 96, height: 96))
            }.value
        }
    }
}
