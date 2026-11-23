#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#include <assert.h>

extern void blink_fog_mask(void *, const unsigned char *, int, int);
static BOOL reduceTransparency;
static BOOL testPreference(id self, SEL selector) { return reduceTransparency; }

int main(void) { @autoreleasepool {
    [NSApplication sharedApplication];
    Method preference = class_getInstanceMethod(NSWorkspace.class, @selector(accessibilityDisplayShouldReduceTransparency));
    IMP original = method_setImplementation(preference, (IMP)testPreference);
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,400,300)
        styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
    window.opaque = NO;
    window.backgroundColor = NSColor.clearColor;
    unsigned char alpha[] = {255,0,0,255};
    reduceTransparency = NO;
    blink_fog_mask((__bridge void *)window, alpha, 2, 2);
    NSVisualEffectView *fog = (id)window.contentView.subviews.lastObject;
    assert([fog isKindOfClass:NSVisualEffectView.class] && !fog.hidden);
    reduceTransparency = YES;
    blink_fog_mask((__bridge void *)window, alpha, 2, 2);
    assert(fog.hidden && "Reduce Transparency must not leave an opaque native backdrop");
    assert(!window.opaque && window.backgroundColor.alphaComponent == 0);
    reduceTransparency = NO;
    blink_fog_mask((__bridge void *)window, alpha, 2, 2);
    assert(!fog.hidden && fog.maskImage != nil);
    blink_fog_mask((__bridge void *)window, NULL, 0, 0);
    assert(fog.hidden);
    method_setImplementation(preference, original);
    puts("PASS: native fog follows accessibility preference changes and clears at round end");
} }
