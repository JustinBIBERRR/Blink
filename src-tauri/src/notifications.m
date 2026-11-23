#import <Foundation/Foundation.h>
#import <UserNotifications/UserNotifications.h>

void blink_silent_notification(const char *body) {
    @autoreleasepool {
        // UNUserNotificationCenter requires a packaged application identity.
        if (!NSBundle.mainBundle.bundleIdentifier) return;
        UNUserNotificationCenter *center = UNUserNotificationCenter.currentNotificationCenter;
        UNMutableNotificationContent *content = [UNMutableNotificationContent new];
        content.title = @"Blink";
        content.body = [NSString stringWithUTF8String:body];
        content.sound = nil;
        // Alert only: never request sound authorization. Delivery is asynchronous,
        // and refusal or Focus filtering cannot delay visual lifecycle transitions.
        [center requestAuthorizationWithOptions:UNAuthorizationOptionAlert completionHandler:^(BOOL granted, NSError *error) {
            if (error || !granted) return;
            UNNotificationRequest *request = [UNNotificationRequest requestWithIdentifier:NSUUID.UUID.UUIDString content:content trigger:nil];
            [center addNotificationRequest:request withCompletionHandler:^(NSError *deliveryError) {
                if (deliveryError) NSLog(@"Blink notification unavailable: %@", deliveryError);
            }];
        }];
    }
}
