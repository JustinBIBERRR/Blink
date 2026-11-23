#import <AppKit/AppKit.h>
// Process-lifetime observers. The distributed lock names are undocumented;
// keep that compatibility bridge isolated and validate on supported macOS builds.
void blink_install_system_events(void (*callback)(int, bool)) {
    NSNotificationCenter *workspace = NSWorkspace.sharedWorkspace.notificationCenter;
    [workspace addObserverForName:NSWorkspaceWillSleepNotification object:nil queue:nil usingBlock:^(NSNotification *n) { callback(1, true); }];
    [workspace addObserverForName:NSWorkspaceDidWakeNotification object:nil queue:nil usingBlock:^(NSNotification *n) { callback(1, false); }];
    NSDistributedNotificationCenter *distributed = NSDistributedNotificationCenter.defaultCenter;
    [distributed addObserverForName:@"com.apple.screenIsLocked" object:nil queue:nil usingBlock:^(NSNotification *n) { callback(0, true); }];
    [distributed addObserverForName:@"com.apple.screenIsUnlocked" object:nil queue:nil usingBlock:^(NSNotification *n) { callback(0, false); }];
}
