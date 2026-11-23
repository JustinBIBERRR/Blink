import AppKit
import Metal
import MetalKit
import MetalPerformanceShaders
import ScreenCaptureKit
import CoreMedia
import CoreVideo
import Darwin

struct Metrics: Encodable {
    let event: String
    let timestamp: Double
    let state: String
    let source: String
    let renderedFrames: Int
    let completedFrames: Int
    let captureFrames: Int
    let replacedFrames: Int
    let skippedDraws: Int
    let gpuErrors: Int
    let frameP50Ms: Double?
    let frameP95Ms: Double?
    let gpuP50Ms: Double?
    let frameAgeMs: Double?
    let inputToSubmitMs: Double?
    let cpuPercent: Double
    let residentMB: Double
    let inFlight: Int
    let texturesHeld: Int
    let captureActive: Bool
    let renderActive: Bool
    let width: Int
    let height: Int
}

final class Journal {
    private let file: FileHandle
    let path: String
    init() throws {
        let args = CommandLine.arguments
        path = args.firstIndex(of: "--log").flatMap { $0 + 1 < args.count ? args[$0 + 1] : nil }
            ?? "/private/tmp/blink-ice-lab-\(Int(Date().timeIntervalSince1970)).jsonl"
        FileManager.default.createFile(atPath: path, contents: nil)
        file = try FileHandle(forWritingTo: URL(fileURLWithPath: path))
    }
    func write(_ metrics: Metrics) {
        if var data = try? JSONEncoder().encode(metrics) { data.append(10); try? file.write(contentsOf: data) }
    }
}

final class CaptureMailbox: NSObject, SCStreamOutput, SCStreamDelegate {
    private let lock = NSLock()
    private var pending: (CVPixelBuffer, Double)?
    private var enabled = true
    private var received = 0
    private var replaced = 0
    var onError: ((Error) -> Void)?
    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, sampleBuffer.isValid,
              let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let rawStatus = attachments.first?[.status] as? Int,
              SCFrameStatus(rawValue: rawStatus) == .complete,
              let pixel = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        lock.lock(); defer { lock.unlock() }
        guard enabled else { return }
        received += 1
        if pending != nil { replaced += 1 }
        pending = (pixel, CACurrentMediaTime())
    }
    func stream(_ stream: SCStream, didStopWithError error: Error) { onError?(error) }
    func take() -> (CVPixelBuffer, Double)? {
        lock.lock(); defer { lock.unlock() }; let value = pending; pending = nil; return value
    }
    func counts() -> (Int,Int) { lock.lock(); defer { lock.unlock() }; return (received,replaced) }
    func close() { lock.lock(); enabled = false; pending = nil; lock.unlock() }
}

final class IceView: MTKView {
    var contact: ((CGPoint) -> Void)?
    var gestureBegan: (() -> Void)?
    override var acceptsFirstResponder: Bool { true }
    override func mouseDown(with event: NSEvent) { gestureBegan?(); report(event) }
    override func mouseDragged(with event: NSEvent) { report(event) }
    private func report(_ event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        contact?(CGPoint(x: point.x / bounds.width, y: 1 - point.y / bounds.height))
    }
}

struct Uniforms { var time: Float; var dissolve: Float; var aspect: Float; var synthetic: Float }

final class IceLabSession: NSObject, MTKViewDelegate {
    enum State: String { case idle, starting, running, stopping, failed }
    enum Source: String { case none, synthetic, live }
    private(set) var state: State = .idle
    private(set) var source: Source = .none
    let view: IceView
    let journal: Journal
    let device: MTLDevice
    let queue: MTLCommandQueue
    let pipeline: MTLRenderPipelineState
    private var frost: MTLTexture?
    private var scene: MTLTexture?
    private var blurredScene: MTLTexture?
    private var sceneDirty = true
    private var touchesDirty = true
    private var captureIsActive = false
    private var lastFailure: String?
    private var touchTexture: MTLTexture?
    private var cvCache: CVMetalTextureCache?
    private var cvFrame: CVMetalTexture?
    private var stream: SCStream?
    private var mailbox: CaptureMailbox?
    private var timer: Timer?
    private var metricsTimer: Timer?
    private var generation = 0
    private var startupTask: Task<Void, Never>?
    private var timeOrigin = CACurrentMediaTime()
    private var lastFrameTime: Double?
    private var latestCaptureTime: Double?
    private var dissolveStart: Double?
    private var lastInput: Double?
    private var inputToSubmitMs: Double?
    private var touchData = [Float](repeating: 0, count: 480 * 300)
    private var lastContact: CGPoint?
    private var intervals: [Double] = []
    private var gpuTimes: [Double] = []
    private var rendered = 0
    private var completedFrames = 0
    private var skipped = 0
    private var gpuErrors = 0
    private var inFlight = 0
    private var finalCaptureCounts = (0,0)
    private var previousCPU = 0.0
    private var lastCPUPercent = 0.0
    private var previousSampleTime = CACurrentMediaTime()
    var status: ((String) -> Void)?
    var metricText: ((String) -> Void)?
    var onSelfTestDone: (() -> Void)?

    init(view: IceView, journal: Journal) throws {
        guard let device = view.device, let queue = device.makeCommandQueue() else { throw LabError.message("Metal unavailable") }
        self.view = view; self.device = device; self.queue = queue; self.journal = journal
        let code = try String(contentsOf: Bundle.main.url(forResource: "Ice", withExtension: "metal")!)
        let library = try device.makeLibrary(source: code, options: nil)
        let descriptor = MTLRenderPipelineDescriptor()
        descriptor.vertexFunction = library.makeFunction(name: "fullscreen")
        descriptor.fragmentFunction = library.makeFunction(name: "ice")
        descriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        pipeline = try device.makeRenderPipelineState(descriptor: descriptor)
        super.init()
        CVMetalTextureCacheCreate(nil, nil, device, nil, &cvCache)
        view.delegate = self; view.contact = { [weak self] point in self?.erase(point) }
        view.gestureBegan = { [weak self] in self?.lastContact = nil }
        view.isPaused = true; view.enableSetNeedsDisplay = false; view.framebufferOnly = true
        previousCPU = processCPU()
        metricsTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.sample("sample") }
        sample("initialized")
    }

    func start(_ requested: Source, displayID: CGDirectDisplayID, startupDelayNs: UInt64 = 0) {
        guard state == .idle else { return }
        lastFailure = nil
        generation += 1
        let run = generation
        state = .starting; source = requested
        resetRound(); finalCaptureCounts = (0,0)
        intervals = []; gpuTimes = []; lastFrameTime = nil; latestCaptureTime = nil
        do {
            let loader = MTKTextureLoader(device: device)
            frost = try loader.newTexture(URL: Bundle.main.url(forResource: "deep-frost", withExtension: "png")!, options: [.SRGB: false])
            let descriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r32Float, width: 480, height: 300, mipmapped: false)
            descriptor.usage = .shaderRead; descriptor.storageMode = .shared
            touchTexture = device.makeTexture(descriptor: descriptor)
            uploadTouches()
            scene = try syntheticTexture(); sceneDirty = true
        } catch { fail(error); return }
        sample("starting")
        startupTask = Task { @MainActor in
            do {
                if startupDelayNs > 0 { try await Task.sleep(nanoseconds: startupDelayNs) }
                guard self.generation == run, self.state == .starting else { return }
                if requested == .synthetic { self.beginDrawing(); return }
                let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
                guard self.generation == run, self.state == .starting else { return }
                guard !content.displays.isEmpty else { throw LabError.message("No capture displays") }
                guard let display = content.displays.first(where: { $0.displayID == displayID }) else { throw LabError.message("Selected display disconnected") }
                let excluded = content.applications.filter { $0.processID == getpid() }
                let filter = SCContentFilter(display: display, excludingApplications: excluded, exceptingWindows: [])
                let config = SCStreamConfiguration()
                let factor = min(1.0, 1600.0 / Double(display.width))
                config.width = Int(Double(display.width)*factor); config.height = Int(Double(display.height)*factor)
                config.minimumFrameInterval = CMTime(value: 1, timescale: 30)
                config.queueDepth = 3; config.showsCursor = false; config.pixelFormat = kCVPixelFormatType_32BGRA
                let mailbox = CaptureMailbox()
                mailbox.onError = { [weak self] error in DispatchQueue.main.async {
                    guard self?.generation == run else { return }; self?.fail(error)
                } }
                let stream = SCStream(filter: filter, configuration: config, delegate: mailbox)
                try stream.addStreamOutput(mailbox, type: .screen, sampleHandlerQueue: DispatchQueue(label: "blink.ice.capture"))
                self.mailbox = mailbox; self.stream = stream
                try await stream.startCapture()
                self.captureIsActive = true
                guard self.generation == run, self.state == .starting else { return }
                self.scene = nil
                self.beginDrawing()
            } catch { if self.generation == run { self.fail(error) } }
        }
    }

    private func beginDrawing() {
        state = .running
        status?(source == .live ? "实时桌面 · 拖动擦拭 · 30 fps 目标" : "合成测试场景 · 不代表实时桌面性能")
        timer = Timer.scheduledTimer(withTimeInterval: 1.0/30, repeats: true) { [weak self] _ in self?.view.draw() }
        sample("running")
    }

    func stop() {
        guard state == .running || state == .starting || state == .failed else { return }
        generation += 1; state = .stopping
        timer?.invalidate(); timer = nil
        mailbox?.close()
        finalCaptureCounts = mailbox?.counts() ?? finalCaptureCounts
        let pendingStart = startupTask
        sample("stopping")
        Task { @MainActor in
            await pendingStart?.value
            self.startupTask = nil
            if let stream = self.stream {
                do { try await stream.stopCapture() }
                catch {
                    self.state = .failed
                    self.status?("停止捕获失败：\(error.localizedDescription)")
                    self.sample("stop-error"); return
                }
            }
            self.stream = nil; self.mailbox = nil
            self.captureIsActive = false
            let deadline = CACurrentMediaTime() + 3
            while self.inFlight > 0 && CACurrentMediaTime() < deadline {
                try? await Task.sleep(nanoseconds: 10_000_000)
            }
            guard self.inFlight == 0 else {
                self.state = .failed; self.sample("gpu-drain-timeout"); return
            }
            self.captureIsActive = false
            self.scene = nil; self.blurredScene = nil; self.frost = nil; self.touchTexture = nil; self.cvFrame = nil
            if let cache = self.cvCache { CVMetalTextureCacheFlush(cache, 0) }
            self.touchData = [Float](repeating: 0, count: 480*300)
            self.latestCaptureTime = nil; self.lastContact = nil
            self.state = .idle
            self.status?(self.lastFailure ?? "已停止 · 捕获与绘制已关闭 · 保留监控采样")
            self.sample("stopped")
        }
    }

    private func fail(_ error: Error) {
        let detail = error as NSError
        let message = "错误：\(detail.localizedDescription) [\(detail.domain):\(detail.code)]"
        lastFailure = message
        status?(message)
        fputs("Ice Lab: \(error)\n", stderr)
        state = .failed; sample("error"); stop()
    }

    func resetRound() {
        timeOrigin = CACurrentMediaTime(); dissolveStart = nil; lastInput = nil; inputToSubmitMs = nil
        touchData = [Float](repeating: 0, count: 480*300); lastContact = nil; uploadTouches()
    }
    func melt() { guard state == .running else { return }; dissolveStart = CACurrentMediaTime(); sample("dissolve") }
    func erase(_ point: CGPoint) {
        guard state == .running else { return }
        let now = CACurrentMediaTime()
        lastInput = now
        let previous = lastContact ?? point; lastContact = point
        let ax = Float(previous.x)*480, ay = Float(previous.y)*300
        let bx = Float(point.x)*480, by = Float(point.y)*300
        let dx = bx-ax, dy = by-ay, length = dx*dx+dy*dy
        let radius: Float = 23
        let left = max(0,Int(min(ax,bx)-radius)), right = min(479,Int(max(ax,bx)+radius))
        let top = max(0,Int(min(ay,by)-radius)), bottom = min(299,Int(max(ay,by)+radius))
        guard left <= right, top <= bottom else { return }
        for y in top...bottom { for x in left...right {
            let px = Float(x)+0.5, py = Float(y)+0.5
            let t = length > 0 ? max(0,min(1,((px-ax)*dx+(py-ay)*dy)/length)) : 0
            let distance = hypot(px-ax-t*dx,py-ay-t*dy)
            if distance < radius && touchData[y*480+x] == 0 {
                touchData[y*480+x] = Float(now-timeOrigin)+1+max(0,(distance/radius-0.75))*0.45
            }
        } }
        uploadTouches()
    }
    private func uploadTouches() { touchesDirty = true }
    private func flushTouches() {
        touchData.withUnsafeBytes { bytes in
            touchTexture?.replace(region: MTLRegionMake2D(0,0,480,300), mipmapLevel: 0, withBytes: bytes.baseAddress!, bytesPerRow: 480*4)
        }
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}
    func draw(in view: MTKView) {
        guard state == .running else { return }
        guard inFlight == 0 else { skipped += 1; return }
        if let (buffer,receivedAt) = mailbox?.take(), let cache = cvCache {
            var frame: CVMetalTexture?
            let code = CVMetalTextureCacheCreateTextureFromImage(nil,cache,buffer,nil,.bgra8Unorm,CVPixelBufferGetWidth(buffer),CVPixelBufferGetHeight(buffer),0,&frame)
            if code == kCVReturnSuccess, let frame, let texture = CVMetalTextureGetTexture(frame) {
                cvFrame = frame; scene = texture; sceneDirty = true; latestCaptureTime = receivedAt
            } else { gpuErrors += 1 }
        }
        guard let scene, let frost, let touches = touchTexture,
              let pass = view.currentRenderPassDescriptor, let drawable = view.currentDrawable,
              let command = queue.makeCommandBuffer() else { skipped += 1; return }
        if touchesDirty { flushTouches(); touchesDirty = false }
        if blurredScene?.width != scene.width || blurredScene?.height != scene.height {
            let description = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: scene.width, height: scene.height, mipmapped: false)
            description.usage = [.shaderRead, .shaderWrite]; description.storageMode = .private
            blurredScene = device.makeTexture(descriptor: description); sceneDirty = true
        }
        guard let blurredScene else { gpuErrors += 1; return }
        if sceneDirty {
            let blur = MPSImageGaussianBlur(device: device, sigma: 3.8)
            blur.edgeMode = .clamp
            blur.encode(commandBuffer: command, sourceTexture: scene, destinationTexture: blurredScene)
            sceneDirty = false
        }
        guard let encoder = command.makeRenderCommandEncoder(descriptor: pass) else { gpuErrors += 1; return }
        let now = CACurrentMediaTime()
        var params = Uniforms(time: Float(now-timeOrigin), dissolve: dissolveStart.map { Float(min(1,(now-$0)/3)) } ?? -1,
                              aspect: Float(view.drawableSize.width/view.drawableSize.height), synthetic: source == .synthetic ? 1 : 0)
        encoder.setRenderPipelineState(pipeline)
        encoder.setFragmentTexture(scene,index:0); encoder.setFragmentTexture(frost,index:1); encoder.setFragmentTexture(touches,index:2); encoder.setFragmentTexture(blurredScene,index:3)
        encoder.setFragmentBytes(&params,length:MemoryLayout<Uniforms>.stride,index:0)
        encoder.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:3); encoder.endEncoding()
        command.present(drawable)
        inFlight += 1; rendered += 1
        if let lastFrameTime { intervals.append((now-lastFrameTime)*1000) }; lastFrameTime = now
        if let input = lastInput { inputToSubmitMs = (now-input)*1000; lastInput = nil }
        let keepFrame = cvFrame
        command.addCompletedHandler { [weak self] completed in
            _ = keepFrame
            let gpuMs = completed.gpuEndTime > completed.gpuStartTime && completed.gpuStartTime > 0 ? (completed.gpuEndTime-completed.gpuStartTime)*1000 : nil
            DispatchQueue.main.async {
                guard let self else { return }
                self.inFlight -= 1
                self.completedFrames += 1
                if completed.status == .error { self.gpuErrors += 1; fputs("Metal command failed: \(String(describing: completed.error))\n",stderr) }
                if let gpuMs { self.gpuTimes.append(gpuMs) }
            }
        }
        command.commit()
    }

    private func processCPU() -> Double {
        var usage = rusage(); getrusage(RUSAGE_SELF,&usage)
        return Double(usage.ru_utime.tv_sec+usage.ru_stime.tv_sec)+Double(usage.ru_utime.tv_usec+usage.ru_stime.tv_usec)/1_000_000
    }
    private func residentMemory() -> Double {
        var info = mach_task_basic_info(); var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size/MemoryLayout<natural_t>.size)
        let result = withUnsafeMutablePointer(to: &info) { ptr in ptr.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
            task_info(mach_task_self_,task_flavor_t(MACH_TASK_BASIC_INFO),$0,&count)
        } }
        return result == KERN_SUCCESS ? Double(info.resident_size)/1_048_576 : -1
    }
    private func percentile(_ values: [Double], _ p: Double) -> Double? {
        guard !values.isEmpty else { return nil }; let sorted = values.sorted(); return sorted[min(sorted.count-1,Int(Double(sorted.count-1)*p))]
    }
    func sample(_ event: String) {
        let now = CACurrentMediaTime(), cpu = processCPU()
        if event == "sample" {
            lastCPUPercent = (cpu-previousCPU)/max(0.001,now-previousSampleTime)*100
            previousCPU = cpu; previousSampleTime = now
        }
        let utilization = lastCPUPercent
        let counts = mailbox?.counts() ?? finalCaptureCounts
        let metrics = Metrics(event:event,timestamp:Date().timeIntervalSince1970,state:state.rawValue,source:source.rawValue,
            renderedFrames:rendered,completedFrames:completedFrames,captureFrames:counts.0,replacedFrames:counts.1,skippedDraws:skipped,gpuErrors:gpuErrors,
            frameP50Ms:percentile(intervals,0.5),frameP95Ms:percentile(intervals,0.95),gpuP50Ms:percentile(gpuTimes,0.5),
            frameAgeMs:latestCaptureTime.map { (now-$0)*1000 },inputToSubmitMs:inputToSubmitMs,
            cpuPercent:utilization,residentMB:residentMemory(),inFlight:inFlight,
            texturesHeld:[scene,blurredScene,frost,touchTexture].compactMap{$0}.count,captureActive:captureIsActive,renderActive:timer != nil,
            width:Int(view.drawableSize.width),height:Int(view.drawableSize.height))
        journal.write(metrics)
        func format(_ n: Double?) -> String { n.map { String(format:"%0.1f",$0) } ?? "n/a" }
        metricText?("\(state.rawValue) / \(source.rawValue)   CPU \(format(utilization))%   RSS \(format(metrics.residentMB)) MB\nFrame p50/p95 \(format(metrics.frameP50Ms))/\(format(metrics.frameP95Ms)) ms   GPU p50 \(format(metrics.gpuP50Ms)) ms   input→submit \(format(inputToSubmitMs)) ms\nDraw \(rendered)   capture \(counts.0)   replaced \(counts.1)   GPU errors \(gpuErrors)   textures \(metrics.texturesHeld)   in-flight \(inFlight)\nLog \(journal.path)")
        intervals.removeAll(keepingCapacity:true); gpuTimes.removeAll(keepingCapacity:true)
    }

    private func syntheticTexture() throws -> MTLTexture {
        let width = 1440, height = 900
        let bitmap = NSBitmapImageRep(bitmapDataPlanes:nil,pixelsWide:width,pixelsHigh:height,bitsPerSample:8,samplesPerPixel:4,hasAlpha:true,isPlanar:false,colorSpaceName:.deviceRGB,bytesPerRow:width*4,bitsPerPixel:32)!
        NSGraphicsContext.saveGraphicsState(); NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep:bitmap)
        NSColor(calibratedRed:0.16,green:0.21,blue:0.26,alpha:1).setFill(); NSRect(x:0,y:0,width:width,height:height).fill()
        let panel = NSRect(x:335,y:230,width:770,height:490)
        NSColor(calibratedWhite:0.94,alpha:1).setFill(); NSBezierPath(roundedRect:panel,xRadius:18,yRadius:18).fill()
        NSColor(calibratedWhite:0.85,alpha:1).setFill(); NSRect(x:335,y:675,width:770,height:45).fill()
        for (index,color) in [NSColor.systemRed,.systemYellow,.systemGreen].enumerated() {
            color.setFill(); NSBezierPath(ovalIn:NSRect(x:355+index*24,y:690,width:13,height:13)).fill()
        }
        func text(_ value: String,_ x: Double,_ y: Double,_ size: Double,_ bold: Bool=false) {
            (value as NSString).draw(at:NSPoint(x:x,y:y),withAttributes:[.font:bold ? NSFont.boldSystemFont(ofSize:size) : NSFont.systemFont(ofSize:size),.foregroundColor:NSColor(calibratedWhite:0.15,alpha:1)])
        }
        text("项目备忘",390,600,34,true)
        text("□  整理需求文档",390,535,24)
        text("□  完成方案初稿",390,480,24)
        text("□  同步评审时间",390,425,24)
        text("冰层材质验收 · 合成背景",390,310,19)
        text("BLINK  /  DEEP WINTER",60,805,22)
        NSGraphicsContext.restoreGraphicsState()
        guard let cg = bitmap.cgImage else { throw LabError.message("Synthetic background failed") }
        return try MTKTextureLoader(device:device).newTexture(cgImage:cg,options:[.SRGB:false])
    }

    func startupStopTest() {
        start(.synthetic, displayID: 0, startupDelayNs: 500_000_000)
        DispatchQueue.main.asyncAfter(deadline: .now()+0.05) { self.stop() }
        DispatchQueue.main.asyncAfter(deadline: .now()+0.10) {
            self.start(.synthetic, displayID: 0)
            precondition(self.state == .stopping, "Restart must wait for pending startup cleanup")
        }
        DispatchQueue.main.asyncAfter(deadline: .now()+1) {
            precondition(self.state == .idle && self.rendered == 0 && self.stream == nil && self.scene == nil,
                         "Cancelled startup must not render or retain resources")
            self.sample("startup-stop-verified")
            self.start(.synthetic, displayID: 0)
        }
        DispatchQueue.main.asyncAfter(deadline: .now()+3) { self.stop() }
        DispatchQueue.main.asyncAfter(deadline: .now()+4) {
            precondition(self.state == .idle && self.rendered > 0 && self.rendered == self.completedFrames && self.inFlight == 0,
                         "Restart after cleanup must render and drain")
            self.sample("startup-stop-test-complete"); self.onSelfTestDone?()
        }
    }

    func selfTest() {
        status?("自动测试 · 先记录 3 秒空闲基线")
        DispatchQueue.main.asyncAfter(deadline:.now()+3) { self.start(.synthetic,displayID:CGMainDisplayID()) }
        DispatchQueue.main.asyncAfter(deadline:.now()+6) { self.erase(CGPoint(x:0.45,y:0.45)); self.sample("scripted-contact") }
        DispatchQueue.main.asyncAfter(deadline:.now()+7) { self.erase(CGPoint(x:0.58,y:0.5)) }
        DispatchQueue.main.asyncAfter(deadline:.now()+9) { self.melt() }
        DispatchQueue.main.asyncAfter(deadline:.now()+13) { self.stop() }
        DispatchQueue.main.asyncAfter(deadline:.now()+18) { self.sample("self-test-complete"); self.onSelfTestDone?() }
    }
}

enum LabError: Error { case message(String) }

final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var window: NSWindow!
    var session: IceLabSession!
    let status = NSTextField(labelWithString:"就绪 · 先选择合成预览或实时桌面")
    let metrics = NSTextField(wrappingLabelWithString:"")
    let displays = NSPopUpButton(frame:.zero,pullsDown:false)
    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            let appMenu = NSMenu(); let mainItem = NSMenuItem(); appMenu.addItem(mainItem)
            let submenu = NSMenu(); submenu.addItem(withTitle:"退出 Blink Ice Lab",action:#selector(NSApplication.terminate(_:)),keyEquivalent:"q"); mainItem.submenu = submenu; NSApp.mainMenu = appMenu
            window = NSWindow(contentRect:NSRect(x:0,y:0,width:1140,height:840),styleMask:[.titled,.closable,.miniaturizable,.resizable],backing:.buffered,defer:false)
            window.title = "Blink Ice Lab · C 质感原型"; window.delegate = self; window.minSize = NSSize(width:850,height:650)
            let root = NSView(); window.contentView = root
            let metal = IceView(frame:.zero,device:MTLCreateSystemDefaultDevice()); metal.colorPixelFormat = .bgra8Unorm
            metal.translatesAutoresizingMaskIntoConstraints = false
            let bar = NSStackView(); bar.orientation = .horizontal; bar.spacing = 8; bar.translatesAutoresizingMaskIntoConstraints = false
            for (title,action) in [("实时桌面",#selector(live)),("合成预览",#selector(synthetic)),("停止",#selector(stop)),("中央消融",#selector(melt)),("重置冰层",#selector(reset)),("显示/隐藏监控",#selector(toggleMetrics))] {
                let button = NSButton(title:title,target:self,action:action); bar.addArrangedSubview(button)
            }
            for (i,screen) in NSScreen.screens.enumerated() { displays.addItem(withTitle:"屏幕 \(i+1) · \(screen.localizedName)"); displays.lastItem?.representedObject = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] }
            bar.addArrangedSubview(displays)
            status.translatesAutoresizingMaskIntoConstraints = false; metrics.translatesAutoresizingMaskIntoConstraints = false
            metrics.font = .monospacedSystemFont(ofSize:11,weight:.regular)
            root.addSubview(bar); root.addSubview(status); root.addSubview(metal); root.addSubview(metrics)
            NSLayoutConstraint.activate([
                bar.leadingAnchor.constraint(equalTo:root.leadingAnchor,constant:16),bar.topAnchor.constraint(equalTo:root.topAnchor,constant:12),
                status.topAnchor.constraint(equalTo:bar.bottomAnchor,constant:10),status.leadingAnchor.constraint(equalTo:bar.leadingAnchor),
                metal.topAnchor.constraint(equalTo:status.bottomAnchor,constant:10),metal.leadingAnchor.constraint(equalTo:root.leadingAnchor,constant:16),metal.trailingAnchor.constraint(equalTo:root.trailingAnchor,constant:-16),
                metrics.topAnchor.constraint(equalTo:metal.bottomAnchor,constant:10),metrics.leadingAnchor.constraint(equalTo:bar.leadingAnchor),metrics.trailingAnchor.constraint(equalTo:root.trailingAnchor,constant:-16),metrics.heightAnchor.constraint(equalToConstant:72),metrics.bottomAnchor.constraint(equalTo:root.bottomAnchor,constant:-12)
            ])
            session = try IceLabSession(view:metal,journal:Journal())
            session.status = { [weak self] text in self?.status.stringValue = text }
            session.metricText = { [weak self] text in self?.metrics.stringValue = text }
            session.onSelfTestDone = { NSApp.terminate(nil) }
            window.center(); window.makeKeyAndOrderFront(nil); NSApp.activate(ignoringOtherApps:true)
            if CommandLine.arguments.contains("--self-test") { session.selfTest() }
            if CommandLine.arguments.contains("--startup-stop-test") { session.startupStopTest() }
        } catch { fputs("Ice Lab initialization failed: \(error)\n",stderr); NSApp.terminate(nil) }
    }
    @objc func live() { session.start(.live,displayID:(displays.selectedItem?.representedObject as? NSNumber)?.uint32Value ?? CGMainDisplayID()) }
    @objc func synthetic() { session.start(.synthetic,displayID:CGMainDisplayID()) }
    @objc func stop() { session.stop() }
    @objc func melt() { session.melt() }
    @objc func reset() { session.resetRound() }
    @objc func toggleMetrics() { metrics.isHidden.toggle() }
    func windowShouldClose(_ sender: NSWindow) -> Bool { session.stop(); NSApp.terminate(nil); return true }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = AppDelegate(); app.delegate = delegate
app.run()
