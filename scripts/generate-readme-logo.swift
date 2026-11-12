#!/usr/bin/env swift
// Run on macOS: swift scripts/generate-readme-logo.swift
// Uses only AppKit/CoreText and the existing repository artwork and font.
import AppKit
import CoreText

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let iconURL = root.appendingPathComponent("assets/icons/app-icon.png")
let fontURL = root.appendingPathComponent("assets/fonts/departure-mono/DepartureMono-1.500.woff2")
let outputURL = root.appendingPathComponent("assets/readme-logo.png")

func fail(_ message: String) -> Never {
    fputs("\(message)\n", stderr)
    exit(1)
}

guard let source = NSBitmapImageRep(data: try Data(contentsOf: iconURL)),
      let icon = NSImage(contentsOf: iconURL),
      let descriptors = CTFontManagerCreateFontDescriptorsFromURL(fontURL as CFURL) as? [CTFontDescriptor],
      let descriptor = descriptors.first else {
    fail("Could not load the repository icon or Departure Mono font.")
}

// Sample the center of the icon's solid teal sun, away from antialiased edges.
guard let teal = source.colorAt(
    x: Int(Double(source.pixelsWide) * 0.29),
    y: Int(Double(source.pixelsHigh) * 0.31)
)?.usingColorSpace(.deviceRGB) else {
    fail("Could not sample the icon's teal accent.")
}
let font = CTFontCreateWithFontDescriptor(descriptor, 110, nil)
let title = NSAttributedString(string: "BITMAN", attributes: [
    NSAttributedString.Key(kCTFontAttributeName as String): font,
    NSAttributedString.Key(kCTForegroundColorAttributeName as String): teal.cgColor,
])
let line = CTLineCreateWithAttributedString(title)
let bounds = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
let titleX: CGFloat = 238
let width = Int(ceil(titleX + bounds.width + 28))
let height = 220
let scale = 2
guard let bitmap = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: width * scale, pixelsHigh: height * scale,
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
), let graphics = NSGraphicsContext(bitmapImageRep: bitmap) else {
    fail("Could not create the logo canvas.")
}
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = graphics
let context = graphics.cgContext
context.clear(CGRect(x: 0, y: 0, width: width * scale, height: height * scale))
context.scaleBy(x: CGFloat(scale), y: CGFloat(scale))
graphics.imageInterpolation = NSImageInterpolation.high
icon.draw(in: CGRect(x: 24, y: 20, width: 180, height: 180))
context.textPosition = CGPoint(x: titleX - bounds.minX, y: CGFloat(height) / 2 - bounds.midY)
CTLineDraw(line, context)
NSGraphicsContext.restoreGraphicsState()

guard let png = bitmap.representation(using: .png, properties: [:]) else {
    fail("Could not encode the logo PNG.")
}
try png.write(to: outputURL, options: .atomic)
print("Generated \(outputURL.path) (\(width * scale) × \(height * scale))")
print(String(format: "Teal: #%02X%02X%02X", Int(round(teal.redComponent * 255)),
             Int(round(teal.greenComponent * 255)), Int(round(teal.blueComponent * 255))))
