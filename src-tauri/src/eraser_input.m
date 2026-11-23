#import <ApplicationServices/ApplicationServices.h>
#import <Foundation/Foundation.h>
// Callback returns whether this exact event belongs to Blink. No locks or IPC here.
extern bool blink_pointer_event(int kind, double x, double y);
extern void blink_modifier_event(unsigned int key, bool down);
extern void blink_pointer_initial(bool down);
extern void blink_keyboard_event(unsigned int key, bool down, unsigned int flags);
static CFMachPortRef tap;
static unsigned int previousFlags;
static unsigned int virtualKey(CGKeyCode code) {
    // Physical ANSI keys match the settings recorder's KeyboardEvent.code.
    switch (code) {
        case 0:return 'A'; case 1:return 'S'; case 2:return 'D'; case 3:return 'F';
        case 4:return 'H'; case 5:return 'G'; case 6:return 'Z'; case 7:return 'X';
        case 8:return 'C'; case 9:return 'V'; case 11:return 'B'; case 12:return 'Q';
        case 13:return 'W'; case 14:return 'E'; case 15:return 'R'; case 16:return 'Y';
        case 17:return 'T'; case 31:return 'O'; case 32:return 'U'; case 34:return 'I';
        case 35:return 'P'; case 37:return 'L'; case 38:return 'J'; case 40:return 'K';
        case 45:return 'N'; case 46:return 'M';
        case 18:return '1'; case 19:return '2'; case 20:return '3'; case 21:return '4';
        case 23:return '5'; case 22:return '6'; case 26:return '7'; case 28:return '8';
        case 25:return '9'; case 29:return '0';
        case 36:return 0x0D; case 48:return 0x09; case 49:return 0x20; case 53:return 0x1B;
        case 123:return 0x25; case 124:return 0x27; case 125:return 0x28; case 126:return 0x26;
        case 122:return 0x70; case 120:return 0x71; case 99:return 0x72; case 118:return 0x73;
        case 96:return 0x74; case 97:return 0x75; case 98:return 0x76; case 100:return 0x77;
        case 101:return 0x78; case 109:return 0x79; case 103:return 0x7A; case 111:return 0x7B;
        default:return 0;
    }
}
static CGEventRef filter(CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *context) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        blink_pointer_event(3, 0, 0);
        for (unsigned int bit = 1; bit <= 8; bit <<= 1) {
            blink_modifier_event(bit == 1 ? 0x11 : bit == 2 ? 0x10 : bit == 4 ? 0x12 : 0x5B, false);
        }
        previousFlags = 0;
        blink_keyboard_event(UINT32_MAX, false, 0);
        CGEventTapEnable(tap, true);
        return event;
    }
    if (type == kCGEventFlagsChanged || type == kCGEventKeyDown || type == kCGEventKeyUp) {
        CGEventFlags f = CGEventGetFlags(event);
        unsigned int flags = ((f & kCGEventFlagMaskControl) ? 1 : 0) |
            ((f & kCGEventFlagMaskShift) ? 2 : 0) | ((f & kCGEventFlagMaskAlternate) ? 4 : 0) |
            ((f & kCGEventFlagMaskCommand) ? 8 : 0);
        for (unsigned int bit = 1; bit <= 8; bit <<= 1) {
            if ((flags ^ previousFlags) & bit)
                blink_modifier_event(bit == 1 ? 0x11 : bit == 2 ? 0x10 : bit == 4 ? 0x12 : 0x5B, (flags & bit) != 0);
        }
        previousFlags = flags;
        blink_keyboard_event(type == kCGEventFlagsChanged ? 0 : virtualKey((CGKeyCode)CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode)), type == kCGEventKeyDown, flags);
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
    // Called after explicit pickup or opting into keyboard shortcuts.
    NSDictionary *options = @{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES};
    if (!AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options)) return false;
    if (tap) return true;
    blink_pointer_initial(CGEventSourceButtonState(kCGEventSourceStateCombinedSessionState, kCGMouseButtonLeft));
    CGEventMask mask = CGEventMaskBit(kCGEventMouseMoved) | CGEventMaskBit(kCGEventLeftMouseDragged) |
        CGEventMaskBit(kCGEventLeftMouseDown) | CGEventMaskBit(kCGEventLeftMouseUp) |
        CGEventMaskBit(kCGEventScrollWheel) | CGEventMaskBit(kCGEventFlagsChanged) |
        CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp);
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
