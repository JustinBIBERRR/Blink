#import <AppKit/AppKit.h>
// Main-thread only. Native behind-window compositing samples the live desktop,
// unlike a WebView's CSS backdrop-filter. Alpha zero is a real clear hole.
void blink_fog_mask(void *raw, const unsigned char *alpha, int width, int height) {
    NSWindow *window = (__bridge NSWindow *)raw;
    NSView *content = window.contentView;
    NSVisualEffectView *fog = nil;
    for (NSView *view in content.subviews) {
        if ([view.identifier isEqualToString:@"blink-fog"]) { fog = (NSVisualEffectView *)view; break; }
    }
    if (!fog && !alpha) return;
    if (!fog) {
        fog = [[NSVisualEffectView alloc] initWithFrame:content.bounds];
        fog.identifier = @"blink-fog";
        fog.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        fog.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        fog.material = NSVisualEffectMaterialUnderWindowBackground;
        fog.state = NSVisualEffectStateActive;
        [content addSubview:fog positioned:NSWindowBelow relativeTo:nil];
    }
    if (!alpha || width <= 0 || height <= 0) { fog.hidden = YES; return; }
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc]
        initWithBitmapDataPlanes:NULL pixelsWide:width pixelsHigh:height
        bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO
        colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:width*4 bitsPerPixel:32];
    unsigned char *pixels = bitmap.bitmapData;
    for (int i=0; i<width*height; i++) {
        pixels[i*4]=pixels[i*4+1]=pixels[i*4+2]=alpha[i]; pixels[i*4+3]=alpha[i];
    }
    NSImage *mask = [[NSImage alloc] initWithSize:content.bounds.size];
    [mask addRepresentation:bitmap];
    fog.maskImage = mask;
    fog.hidden = NO;
}
