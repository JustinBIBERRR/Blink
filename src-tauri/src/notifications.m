#import <Foundation/Foundation.h>
#import <UserNotifications/UserNotifications.h>

@interface BlinkNotificationDelegate : NSObject <UNUserNotificationCenterDelegate>
@end
@implementation BlinkNotificationDelegate
- (void)userNotificationCenter:(UNUserNotificationCenter *)center
      willPresentNotification:(UNNotification *)notification
        withCompletionHandler:(void (^)(UNNotificationPresentationOptions))completion {
    (void)center; (void)notification;
    completion(UNNotificationPresentationOptionBanner | UNNotificationPresentationOptionList);
}
@end

void blink_silent_notification(const char *body) {
    @autoreleasepool {
        // UNUserNotificationCenter requires a packaged application identity.
        if (!NSBundle.mainBundle.bundleIdentifier) return;
        UNUserNotificationCenter *center = UNUserNotificationCenter.currentNotificationCenter;
        static BlinkNotificationDelegate *delegate;
        static dispatch_once_t once;
        dispatch_once(&once, ^{ delegate = [BlinkNotificationDelegate new]; });
        center.delegate = delegate;
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
