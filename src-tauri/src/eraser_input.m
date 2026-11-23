#import <ApplicationServices/ApplicationServices.h>
#import <Foundation/Foundation.h>
// Callback returns whether this exact event belongs to Blink. No locks or IPC here.
extern bool blink_pointer_event(int kind, double x, double y);
static CFMachPortRef tap;
static CGEventRef filter(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *context) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        blink_pointer_event(3, 0, 0);
        CGEventTapEnable(tap, true);
        return event;
    }
    CGPoint p = CGEventGetLocation(event);
    int kind = type == kCGEventLeftMouseDown ? 1 : type == kCGEventLeftMouseUp ? 2 : 0;
    return blink_pointer_event(kind, p.x, p.y) ? NULL : event;
}
bool blink_start_pointer(void) {
    // Called only after the user explicitly picks up the enabled eraser.
    NSDictionary *options = @{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES};
    if (!AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options)) return false;
    if (tap) return true;
    CGEventMask mask = CGEventMaskBit(kCGEventMouseMoved) | CGEventMaskBit(kCGEventLeftMouseDragged) |
        CGEventMaskBit(kCGEventLeftMouseDown) | CGEventMaskBit(kCGEventLeftMouseUp);
    tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, filter, NULL);
    if (!tap) return false;
    CFRunLoopSourceRef source = CFMachPortCreateRunLoopSource(NULL, tap, 0);
    CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
    CFRelease(source);
    CGEventTapEnable(tap, true);
    return true;
}
#import <AppKit/AppKit.h>
void blink_window_bounds(void *raw, double *bounds) {
    NSWindow *window = (__bridge NSWindow *)raw;
    NSRect r = window.frame;
    double top = NSMaxY(NSScreen.screens.firstObject.frame);
    bounds[0] = r.origin.x; bounds[1] = top - NSMaxY(r);
    bounds[2] = NSMaxX(r); bounds[3] = top - r.origin.y;
}
