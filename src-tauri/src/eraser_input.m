#import <ApplicationServices/ApplicationServices.h>
#import <Foundation/Foundation.h>
// Callback returns whether this exact event belongs to Blink. No locks or IPC here.
extern bool blink_pointer_event(int kind, double x, double y);
extern void blink_modifier_event(unsigned int key, bool down);
extern void blink_pointer_initial(bool down);
static CFMachPortRef tap;
static unsigned int previousFlags;
static CGEventRef filter(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *context) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        blink_pointer_event(3, 0, 0);
        for (unsigned int bit = 1; bit <= 8; bit <<= 1) {
            blink_modifier_event(bit == 1 ? 0x11 : bit == 2 ? 0x10 : bit == 4 ? 0x12 : 0x5B, false);
        }
        previousFlags = 0;
        CGEventTapEnable(tap, true);
        return event;
    }
    if (type == kCGEventFlagsChanged) {
        CGEventFlags f = CGEventGetFlags(event);
        unsigned int flags = ((f & kCGEventFlagMaskControl) ? 1 : 0) |
            ((f & kCGEventFlagMaskShift) ? 2 : 0) | ((f & kCGEventFlagMaskAlternate) ? 4 : 0) |
            ((f & kCGEventFlagMaskCommand) ? 8 : 0);
        for (unsigned int bit = 1; bit <= 8; bit <<= 1) {
            if ((flags ^ previousFlags) & bit)
                blink_modifier_event(bit == 1 ? 0x11 : bit == 2 ? 0x10 : bit == 4 ? 0x12 : 0x5B, (flags & bit) != 0);
        }
        previousFlags = flags;
        return event;
    }
    if (type == kCGEventScrollWheel) {
        double delta = CGEventGetDoubleValueField(event, kCGScrollWheelEventDeltaAxis1);
        if (delta == 0) delta = CGEventGetDoubleValueField(event, kCGScrollWheelEventDeltaAxis2);
        return blink_pointer_event(4, delta, 0) ? NULL : event;
    }
    CGPoint p = CGEventGetLocation(event);
    int kind = type == kCGEventLeftMouseDown ? 1 : type == kCGEventLeftMouseUp ? 2 : 0;
    return blink_pointer_event(kind, p.x, p.y) ? NULL : event;
}
bool blink_start_pointer(void) {
    // Called after explicit pickup or enabling temporary modifier pickup.
    NSDictionary *options = @{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES};
    if (!AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options)) return false;
    if (tap) return true;
    blink_pointer_initial(CGEventSourceButtonState(kCGEventSourceStateCombinedSessionState, kCGMouseButtonLeft));
    CGEventMask mask = CGEventMaskBit(kCGEventMouseMoved) | CGEventMaskBit(kCGEventLeftMouseDragged) |
        CGEventMaskBit(kCGEventLeftMouseDown) | CGEventMaskBit(kCGEventLeftMouseUp) |
        CGEventMaskBit(kCGEventScrollWheel) | CGEventMaskBit(kCGEventFlagsChanged);
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
