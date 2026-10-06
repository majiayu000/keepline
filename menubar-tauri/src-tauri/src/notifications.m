#import <Foundation/Foundation.h>
#import <UserNotifications/UserNotifications.h>

static void (*openLedger)(const char *, const char *);
@interface KeeplineNotificationDelegate : NSObject <UNUserNotificationCenterDelegate>
@end
@implementation KeeplineNotificationDelegate
- (void)userNotificationCenter:(UNUserNotificationCenter *)center willPresentNotification:(UNNotification *)notification withCompletionHandler:(void (^)(UNNotificationPresentationOptions))completion {
  if (@available(macOS 11.0, *)) completion(UNNotificationPresentationOptionBanner | UNNotificationPresentationOptionSound);
  else completion(UNNotificationPresentationOptionAlert | UNNotificationPresentationOptionSound);
}
- (void)userNotificationCenter:(UNUserNotificationCenter *)center didReceiveNotificationResponse:(UNNotificationResponse *)response withCompletionHandler:(void (^)(void))completion {
  NSDictionary *info = response.notification.request.content.userInfo;
  NSString *session = info[@"sessionId"], *anchor = info[@"anchor"];
  if (openLedger && session) openLedger(session.UTF8String, (anchor ?: @"current-step").UTF8String);
  completion();
}
@end
static KeeplineNotificationDelegate *delegate;
void keepline_notifications_init(void (*callback)(const char *, const char *)) {
  openLedger = callback;
  delegate = [KeeplineNotificationDelegate new];
  UNUserNotificationCenter *center = UNUserNotificationCenter.currentNotificationCenter;
  center.delegate = delegate;
  [center requestAuthorizationWithOptions:(UNAuthorizationOptionAlert | UNAuthorizationOptionSound) completionHandler:^(BOOL granted, NSError *error) {
    (void)granted;
    if (error) NSLog(@"Keepline notification authorization failed: %@", error.localizedDescription);
  }];
}
BOOL keepline_notify(const char *id, const char *body, const char *session, const char *anchor, BOOL sound) {
  @autoreleasepool {
    UNMutableNotificationContent *content = [UNMutableNotificationContent new];
    content.title = @"Keepline";
    content.body = [NSString stringWithUTF8String:body];
    content.userInfo = @{ @"sessionId": [NSString stringWithUTF8String:session], @"anchor": [NSString stringWithUTF8String:anchor] };
    if (sound) content.sound = UNNotificationSound.defaultSound;
    UNNotificationRequest *request = [UNNotificationRequest requestWithIdentifier:[NSString stringWithUTF8String:id] content:content trigger:nil];
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    __block BOOL success = NO;
    [UNUserNotificationCenter.currentNotificationCenter addNotificationRequest:request withCompletionHandler:^(NSError *error) {
      success = error == nil;
      if (error) NSLog(@"Keepline notification failed: %@", error.localizedDescription);
      dispatch_semaphore_signal(done);
    }];
    if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC)) != 0) return NO;
    return success;
  }
}
void keepline_notification_clear(const char *id) {
  NSString *identifier = [NSString stringWithUTF8String:id];
  [UNUserNotificationCenter.currentNotificationCenter removeDeliveredNotificationsWithIdentifiers:@[identifier]];
  [UNUserNotificationCenter.currentNotificationCenter removePendingNotificationRequestsWithIdentifiers:@[identifier]];
}
