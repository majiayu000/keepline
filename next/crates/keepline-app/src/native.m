#import <AppKit/AppKit.h>
#import <UserNotifications/UserNotifications.h>
#import <os/log.h>

typedef void (*KLCallback)(const char *);
@interface KLDesktop : NSObject <UNUserNotificationCenterDelegate>
@property NSStatusItem *item;
@property NSPopover *popover;
@property NSDictionary *data;
@property KLCallback callback;
@end
static KLDesktop *desktop;
// Audit only routing identifiers, never notification text or conversation content.
static os_log_t notification_log(void) {
    static os_log_t log; static dispatch_once_t once;
    dispatch_once(&once, ^{log=os_log_create("local.keepline.desktop", "notifications");});
    return log;
}
void kl_desktop_route_applied(const char *session,const char *terminal,const char *tab) {
    os_log(notification_log(), "route_applied bundle=%{public}@ session=%{public}s terminal=%{public}s tab=%{public}s", NSBundle.mainBundle.bundleIdentifier,session,terminal,tab);
}
static NSColor *color(NSNumber *value) {
    unsigned n=value.unsignedIntValue;
    return [NSColor colorWithRed:((n>>16)&255)/255.0 green:((n>>8)&255)/255.0 blue:(n&255)/255.0 alpha:1];
}
@implementation KLDesktop
- (void)route:(NSString *)route { if(self.callback) self.callback(route.UTF8String); }
- (void)choose:(NSButton *)button { [self.popover close]; [self route:button.identifier]; }
- (void)show:(id)sender {
    if(self.popover.shown) {[self.popover close];return;}
    [self rebuild];
    [self.popover showRelativeToRect:self.item.button.bounds ofView:self.item.button preferredEdge:NSRectEdgeMinY];
}
- (void)rebuild {
    NSDictionary *t=self.data[@"tokens"];
    CGFloat pad=[t[@"padding"] doubleValue], width=[t[@"width"] doubleValue], height=[t[@"height"] doubleValue];
    NSViewController *vc=[NSViewController new];vc.view=[[NSView alloc]initWithFrame:NSMakeRect(0,0,width,height)];
    vc.view.wantsLayer=YES;vc.view.layer.backgroundColor=color(t[@"background"]).CGColor;
    NSStackView *stack=[NSStackView new];stack.orientation=NSUserInterfaceLayoutOrientationVertical;stack.alignment=NSLayoutAttributeLeading;stack.spacing=[t[@"gap"] doubleValue];stack.translatesAutoresizingMaskIntoConstraints=NO;
    [vc.view addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[[stack.leadingAnchor constraintEqualToAnchor:vc.view.leadingAnchor constant:pad],[stack.trailingAnchor constraintEqualToAnchor:vc.view.trailingAnchor constant:-pad],[stack.topAnchor constraintEqualToAnchor:vc.view.topAnchor constant:pad]]];
    NSTextField *heading=[NSTextField labelWithString:self.data[@"heading"]?:@"Keepline"];
    heading.font=[NSFont boldSystemFontOfSize:[t[@"title"] doubleValue]];heading.textColor=color(t[@"text"]);[stack addArrangedSubview:heading];
    NSArray *rows=self.data[@"rows"]?:@[];
    for(NSDictionary *row in rows) {
        NSString *title=[NSString stringWithFormat:@"%@\n%@",row[@"title"],row[@"summary"]];
        NSButton *button=[NSButton buttonWithTitle:title target:self action:@selector(choose:)];
        button.identifier=row[@"id"];button.bordered=NO;button.alignment=NSTextAlignmentLeft;
        button.font=[NSFont systemFontOfSize:[t[@"font"] doubleValue]];
        button.contentTintColor=color(t[@"text"]);button.cell.wraps=YES;button.cell.lineBreakMode=NSLineBreakByTruncatingTail;
        button.toolTip=title;[button setAccessibilityLabel:title];[stack addArrangedSubview:button];
        [button.widthAnchor constraintEqualToAnchor:stack.widthAnchor].active=YES;
        [button.heightAnchor constraintEqualToConstant:[t[@"row"] doubleValue]].active=YES;
    }
    NSTextField *summary=[NSTextField wrappingLabelWithString:self.data[@"summary"]?:@""];
    summary.textColor=color(t[@"secondary"]);summary.font=[NSFont systemFontOfSize:[t[@"font"] doubleValue]];[stack addArrangedSubview:summary];
    NSButton *open=[NSButton buttonWithTitle:@"打开工作台" target:self action:@selector(choose:)];open.identifier=@"";open.bezelStyle=NSBezelStyleRounded;[stack addArrangedSubview:open];
    self.popover.contentViewController=vc;self.popover.contentSize=NSMakeSize(width,height);
}
- (void)userNotificationCenter:(UNUserNotificationCenter *)center willPresentNotification:(UNNotification *)notification withCompletionHandler:(void (^)(UNNotificationPresentationOptions))completion {
    os_log(notification_log(), "will_present bundle=%{public}@ request=%{public}@", NSBundle.mainBundle.bundleIdentifier,notification.request.identifier);
    completion(UNNotificationPresentationOptionBanner|UNNotificationPresentationOptionSound);
}
- (void)userNotificationCenter:(UNUserNotificationCenter *)center didReceiveNotificationResponse:(UNNotificationResponse *)response withCompletionHandler:(void (^)(void))completion {
    NSString *sid=response.notification.request.content.userInfo[@"session"];
    os_log(notification_log(), "did_receive bundle=%{public}@ request=%{public}@ action=%{public}@", NSBundle.mainBundle.bundleIdentifier,response.notification.request.identifier,response.actionIdentifier);
    if(sid) dispatch_async(dispatch_get_main_queue(), ^{[self route:sid];});
    completion();
}
@end
void kl_desktop_install(KLCallback callback) {
    desktop=[KLDesktop new];desktop.callback=callback;
    desktop.item=[[NSStatusBar systemStatusBar]statusItemWithLength:NSVariableStatusItemLength];
    desktop.item.button.title=@"K";desktop.item.button.toolTip=@"Keepline";
    desktop.item.button.target=desktop;desktop.item.button.action=@selector(show:);
    desktop.popover=[NSPopover new];desktop.popover.behavior=NSPopoverBehaviorTransient;
    // Unbundled developer binaries cannot use UNUserNotificationCenter. The local .app can.
    if(NSBundle.mainBundle.bundleIdentifier) [UNUserNotificationCenter currentNotificationCenter].delegate=desktop;
}
void kl_desktop_update(const char *json) {
    NSData *bytes=[[NSString stringWithUTF8String:json]dataUsingEncoding:NSUTF8StringEncoding];
    NSError *error=nil;NSDictionary *data=[NSJSONSerialization JSONObjectWithData:bytes options:0 error:&error];
    if(!data) {[desktop route:[@"error:" stringByAppendingString:error.localizedDescription]];return;}
    desktop.data=data;desktop.item.button.title=data[@"badge"]?:@"K";
    if(desktop.popover.shown) [desktop rebuild];
}
void kl_notifications_enable(void) {
    if(!NSBundle.mainBundle.bundleIdentifier) {[desktop route:@"error:系统通知需要从 Keepline.app 启动"];return;}
    [[UNUserNotificationCenter currentNotificationCenter]requestAuthorizationWithOptions:UNAuthorizationOptionAlert|UNAuthorizationOptionSound completionHandler:^(BOOL granted,NSError *error){
        NSString *message=error?[@"error:" stringByAppendingString:error.localizedDescription]:(granted?@"notice:系统通知已允许":@"error:系统未允许通知，请在系统设置中手动打开 Keepline 通知；工作台列表仍可使用");
        dispatch_async(dispatch_get_main_queue(),^{[desktop route:message];});
    }];
}
void kl_desktop_notify(const char *session,const char *title,const char *body) {
    if(!NSBundle.mainBundle.bundleIdentifier) {[desktop route:@"error:系统通知需要从 Keepline.app 启动"];return;}
    NSString *sid=[NSString stringWithUTF8String:session];
    UNMutableNotificationContent *content=[UNMutableNotificationContent new];content.title=[NSString stringWithUTF8String:title];content.body=[NSString stringWithUTF8String:body];content.userInfo=@{@"session":sid};
    UNNotificationRequest *request=[UNNotificationRequest requestWithIdentifier:sid content:content trigger:nil];
    [[UNUserNotificationCenter currentNotificationCenter]addNotificationRequest:request withCompletionHandler:^(NSError *error){
        os_log(notification_log(), "request_completed bundle=%{public}@ request=%{public}@ accepted=%{public}d error_domain=%{public}@ error_code=%{public}ld", NSBundle.mainBundle.bundleIdentifier,sid,error==nil,error.domain?:@"",(long)error.code);
        if(error) dispatch_async(dispatch_get_main_queue(),^{[desktop route:[@"error:" stringByAppendingString:error.localizedDescription]];});
        // Acceptance is not proof of delivery. Record only entries returned by the OS.
        [[UNUserNotificationCenter currentNotificationCenter]getDeliveredNotificationsWithCompletionHandler:^(NSArray<UNNotification *> *notifications){
            for(UNNotification *notification in notifications) if([notification.request.identifier isEqualToString:sid])
                os_log(notification_log(), "delivered_list bundle=%{public}@ request=%{public}@ delivered_at=%{public}@", NSBundle.mainBundle.bundleIdentifier,sid,notification.date);
        }];
    }];
}

void kl_desktop_show(void) { [desktop show:nil]; }
