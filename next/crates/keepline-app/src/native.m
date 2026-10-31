#import <AppKit/AppKit.h>
#import <UserNotifications/UserNotifications.h>
#import <os/log.h>

typedef void (*KLCallback)(const char *);
@class KLRow;
@interface KLDesktop : NSObject <UNUserNotificationCenterDelegate>
@property NSStatusItem *item;
@property NSPopover *popover;
@property NSDictionary *data;
@property KLCallback callback;
@property NSArray<KLRow *> *rows;
- (void)open:(NSString *)route;
- (void)relight;
- (void)focusRow:(NSInteger)offset from:(KLRow *)row;
@end
@interface KLRoot : NSView
@property (weak) KLDesktop *owner;
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
    return [NSColor colorWithSRGBRed:((n>>16)&255)/255.0 green:((n>>8)&255)/255.0 blue:(n&255)/255.0 alpha:1];
}
// The panel follows the system's light or dark mode; the workbench window keeps its own.
static BOOL dark_mode(void) {
    NSAppearanceName name=[NSApp.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua,NSAppearanceNameDarkAqua]];
    return [name isEqualToString:NSAppearanceNameDarkAqua];
}
static NSString *text(id value) { return [value isKindOfClass:NSString.class]?value:@""; }
static NSTextField *label(NSString *string,CGFloat size,NSFontWeight weight,NSColor *ink) {
    NSTextField *field=[NSTextField labelWithString:string?:@""];
    field.font=[NSFont systemFontOfSize:size weight:weight];field.textColor=ink;
    field.lineBreakMode=NSLineBreakByTruncatingTail;
    [field setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow forOrientation:NSLayoutConstraintOrientationHorizontal];
    return field;
}
static NSView *fill(NSColor *background,CGFloat radius) {
    NSView *view=[NSView new];view.wantsLayer=YES;view.translatesAutoresizingMaskIntoConstraints=NO;
    view.layer.backgroundColor=background.CGColor;view.layer.cornerRadius=radius;
    return view;
}
static NSView *hairline(NSColor *ink) {
    NSView *line=fill(ink,0);[line.heightAnchor constraintEqualToConstant:1].active=YES;return line;
}
static NSView *dot(NSColor *ink) {
    NSView *view=fill(ink,3.5);
    [NSLayoutConstraint activateConstraints:@[[view.widthAnchor constraintEqualToConstant:7],[view.heightAnchor constraintEqualToConstant:7]]];
    return view;
}
static NSStackView *line_of(NSArray<NSView *> *views,CGFloat spacing) {
    NSStackView *stack=[NSStackView stackViewWithViews:views];stack.orientation=NSUserInterfaceLayoutOrientationHorizontal;
    stack.alignment=NSLayoutAttributeCenterY;stack.spacing=spacing;stack.translatesAutoresizingMaskIntoConstraints=NO;
    return stack;
}
static NSStackView *column_of(NSArray<NSView *> *views,CGFloat spacing,NSEdgeInsets insets) {
    NSStackView *stack=[NSStackView stackViewWithViews:views];stack.orientation=NSUserInterfaceLayoutOrientationVertical;
    stack.alignment=NSLayoutAttributeLeading;stack.spacing=spacing;stack.edgeInsets=insets;stack.translatesAutoresizingMaskIntoConstraints=NO;
    return stack;
}
// Left text and a quiet note pushed to the right edge, as in the head and section lines.
static NSStackView *spread(NSView *left,NSView *right) {
    [right setContentHuggingPriority:NSLayoutPriorityDefaultLow-1 forOrientation:NSLayoutConstraintOrientationHorizontal];
    if([right isKindOfClass:NSTextField.class]) ((NSTextField *)right).alignment=NSTextAlignmentRight;
    NSStackView *stack=line_of(@[left,right],8);stack.alignment=NSLayoutAttributeFirstBaseline;
    return stack;
}
static void add(NSStackView *stack,NSView *view) {
    [stack addArrangedSubview:view];
    [view.widthAnchor constraintEqualToAnchor:stack.widthAnchor constant:-(stack.edgeInsets.left+stack.edgeInsets.right)].active=YES;
}
static NSButton *text_button(NSString *title,NSColor *ink,CGFloat size,BOOL underline,id target,SEL action,NSString *route) {
    NSButton *button=[NSButton buttonWithTitle:title target:target action:action];button.bordered=NO;button.identifier=route;
    NSMutableDictionary *attributes=[@{NSForegroundColorAttributeName:ink,NSFontAttributeName:[NSFont systemFontOfSize:size]} mutableCopy];
    if(underline) attributes[NSUnderlineStyleAttributeName]=@(NSUnderlineStyleSingle);
    button.attributedTitle=[[NSAttributedString alloc]initWithString:title attributes:attributes];
    return button;
}
/// The offline badge: a hollow ring before the K, drawn in the menu bar's own ink.
static NSImage *offline_mark(void) {
    NSImage *image=[NSImage imageWithSize:NSMakeSize(9,9) flipped:NO drawingHandler:^BOOL(NSRect rect){
        NSBezierPath *ring=[NSBezierPath bezierPathWithOvalInRect:NSInsetRect(rect,1.25,1.25)];ring.lineWidth=1.5;
        [NSColor.blackColor setStroke];[ring stroke];return YES;
    }];
    image.template=YES;return image;
}

// One row that needs you: hover or keyboard focus lays the only background.
@interface KLRow : NSView
@property NSString *route;
@property NSView *line;
@property NSColor *hover;
@property BOOL pointed;
@property BOOL focused;
@property (weak) KLDesktop *owner;
@end
@interface KLDashed : NSView
@property NSColor *edge;
@end
@implementation KLDashed
- (void)drawRect:(NSRect)dirty {
    NSBezierPath *edge=[NSBezierPath bezierPathWithRoundedRect:NSInsetRect(self.bounds,.5,.5) xRadius:8 yRadius:8];
    CGFloat dash[]={3,3};[edge setLineDash:dash count:2 phase:0];edge.lineWidth=1;
    [self.edge setStroke];[edge stroke];
}
@end

@implementation KLDesktop
- (void)route:(NSString *)route { if(self.callback) self.callback(route.UTF8String); }
- (void)open:(NSString *)route { [self.popover close]; [self route:route]; }
- (void)choose:(NSButton *)button { [self open:button.identifier]; }
- (void)copyError:(NSButton *)button {
    NSString *error=text(self.data[@"connection"][@"error"]);
    [NSPasteboard.generalPasteboard clearContents];[NSPasteboard.generalPasteboard setString:error forType:NSPasteboardTypeString];
    [self route:@"notice:已复制错误"];
}
- (void)show:(id)sender {
    if(self.popover.shown) {[self.popover close];return;}
    [self rebuild];
    [self.popover showRelativeToRect:self.item.button.bounds ofView:self.item.button preferredEdge:NSRectEdgeMinY];
    NSView *root=self.popover.contentViewController.view;[root.window makeFirstResponder:root];
}
- (void)observeValueForKeyPath:(NSString *)keyPath ofObject:(id)object change:(NSDictionary *)change context:(void *)context {
    if(self.popover.shown) [self rebuild];
}
// Rows that are lit drop the hairlines on both of their edges.
- (void)relight {
    for(NSUInteger i=0;i<self.rows.count;i++) {
        KLRow *row=self.rows[i];BOOL lit=row.pointed||row.focused;
        row.layer.backgroundColor=lit?row.hover.CGColor:NSColor.clearColor.CGColor;
        row.line.hidden=i==0||lit||self.rows[i-1].pointed||self.rows[i-1].focused;
    }
}
- (void)focusRow:(NSInteger)offset from:(KLRow *)row {
    NSInteger i=row?(NSInteger)[self.rows indexOfObject:row]+offset:0;
    if(i>=0&&i<(NSInteger)self.rows.count) [self.rows[i].window makeFirstResponder:self.rows[i]];
}
- (KLRow *)row:(NSDictionary *)row tokens:(NSDictionary *)t palette:(NSDictionary *)p {
    KLRow *view=[KLRow new];view.owner=self;view.route=row[@"id"];view.hover=color(p[@"hover"]);
    view.wantsLayer=YES;view.layer.cornerRadius=[t[@"radius"] doubleValue];view.translatesAutoresizingMaskIntoConstraints=NO;
    BOOL waits=[row[@"kind"] isEqualToString:@"wait"];
    NSView *mark=dot(color(p[waits?@"wait":@"error"]));
    NSTextField *title=label(row[@"title"],[t[@"font"] doubleValue],NSFontWeightSemibold,color(p[@"text"]));
    NSTextField *age=label(row[@"age"],[t[@"meta"] doubleValue],NSFontWeightRegular,color(p[@"muted"]));
    age.font=[NSFont monospacedDigitSystemFontOfSize:[t[@"meta"] doubleValue] weight:NSFontWeightRegular];age.alignment=NSTextAlignmentRight;
    // Measured, not intrinsic: the label's own size cut short ages such as 「2 分钟」.
    [age.widthAnchor constraintGreaterThanOrEqualToConstant:ceil([age.stringValue sizeWithAttributes:@{NSFontAttributeName:age.font}].width)+4].active=YES;
    NSTextField *summary=label(row[@"summary"],[t[@"small"] doubleValue],NSFontWeightRegular,color(p[waits?@"wait_ink":@"error_ink"]));
    NSTextField *meta=label(text(row[@"meta"]),[t[@"chip"] doubleValue],NSFontWeightRegular,color(p[@"muted"]));
    view.line=hairline(color(p[@"border"]));
    for(NSView *v in @[view.line,mark,title,age,summary,meta]) {v.translatesAutoresizingMaskIntoConstraints=NO;[view addSubview:v];}
    [NSLayoutConstraint activateConstraints:@[
        [view.line.topAnchor constraintEqualToAnchor:view.topAnchor],[view.line.leadingAnchor constraintEqualToAnchor:view.leadingAnchor],[view.line.trailingAnchor constraintEqualToAnchor:view.trailingAnchor],
        [mark.leadingAnchor constraintEqualToAnchor:view.leadingAnchor constant:8],[mark.centerYAnchor constraintEqualToAnchor:title.centerYAnchor],
        [title.leadingAnchor constraintEqualToAnchor:mark.trailingAnchor constant:9],[title.topAnchor constraintEqualToAnchor:view.topAnchor constant:8],
        [age.leadingAnchor constraintGreaterThanOrEqualToAnchor:title.trailingAnchor constant:8],[age.trailingAnchor constraintEqualToAnchor:view.trailingAnchor constant:-8],
        [age.firstBaselineAnchor constraintEqualToAnchor:title.firstBaselineAnchor],
        [summary.leadingAnchor constraintEqualToAnchor:title.leadingAnchor],[summary.trailingAnchor constraintLessThanOrEqualToAnchor:view.trailingAnchor constant:-8],
        [summary.topAnchor constraintEqualToAnchor:title.bottomAnchor constant:2],
        [meta.leadingAnchor constraintEqualToAnchor:title.leadingAnchor],[meta.trailingAnchor constraintLessThanOrEqualToAnchor:view.trailingAnchor constant:-8],
        [meta.topAnchor constraintEqualToAnchor:summary.bottomAnchor constant:2],[meta.bottomAnchor constraintEqualToAnchor:view.bottomAnchor constant:-9],
    ]];
    NSString *spoken=[NSString stringWithFormat:@"%@，%@，%@，%@",row[@"title"],row[@"summary"],row[@"age"],text(row[@"meta"])];
    view.toolTip=spoken;[view setAccessibilityElement:YES];[view setAccessibilityRole:NSAccessibilityButtonRole];[view setAccessibilityLabel:spoken];
    return view;
}
- (void)rebuild {
    NSDictionary *t=self.data[@"tokens"];
    BOOL dark=dark_mode();NSDictionary *p=t[dark?@"dark":@"light"];
    BOOL offline=[self.data[@"connection"][@"error"] isKindOfClass:NSString.class];
    CGFloat width=[t[@"width"] doubleValue], height=[t[@"height"] doubleValue], gap=[t[@"gap"] doubleValue], radius=[t[@"radius"] doubleValue];
    CGFloat small=[t[@"small"] doubleValue], meta=[t[@"meta"] doubleValue], chip=[t[@"chip"] doubleValue];
    NSColor *ink=color(p[@"text"]), *secondary=color(p[@"secondary"]), *muted=color(p[@"muted"]), *border=color(p[@"border"]);
    self.popover.appearance=[NSAppearance appearanceNamed:dark?NSAppearanceNameDarkAqua:NSAppearanceNameAqua];
    NSViewController *vc=[NSViewController new];KLRoot *root=[[KLRoot alloc]initWithFrame:NSMakeRect(0,0,width,height)];root.owner=self;vc.view=root;
    vc.view.wantsLayer=YES;vc.view.layer.backgroundColor=color(p[@"background"]).CGColor;
    NSStackView *stack=column_of(@[],gap,NSEdgeInsetsMake(14,12,12,12));
    [vc.view addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[[stack.leadingAnchor constraintEqualToAnchor:vc.view.leadingAnchor],[stack.trailingAnchor constraintEqualToAnchor:vc.view.trailingAnchor],[stack.topAnchor constraintEqualToAnchor:vc.view.topAnchor]]];
    // One number for the title; the minute it is from when the host is gone.
    NSStackView *head=spread(label(self.data[@"heading"]?:@"Keepline",[t[@"title"] doubleValue],NSFontWeightSemibold,ink),label(text(self.data[@"when"]),meta,NSFontWeightRegular,muted));
    head.edgeInsets=NSEdgeInsetsMake(0,4,0,4);add(stack,head);
    if(offline) {
        NSView *alert=fill(color(p[@"error_wash"]),radius);
        NSImageView *icon=[NSImageView imageViewWithImage:[NSImage imageWithSystemSymbolName:@"exclamationmark.circle" accessibilityDescription:nil]];
        icon.symbolConfiguration=[NSImageSymbolConfiguration configurationWithPointSize:12 weight:NSFontWeightSemibold];icon.contentTintColor=color(p[@"error"]);
        NSTextField *detail=[NSTextField wrappingLabelWithString:text(self.data[@"connection"][@"error"])];
        detail.font=[NSFont monospacedSystemFontOfSize:meta weight:NSFontWeightRegular];detail.textColor=secondary;detail.selectable=YES;
        NSStackView *links=line_of(@[text_button(@"复制错误",ink,small,YES,self,@selector(copyError:),@""),text_button(@"查看诊断",ink,small,YES,self,@selector(choose:),@"diagnostics:")],12);
        NSStackView *body=column_of(@[label(@"正在重连",12.5,NSFontWeightSemibold,ink),detail,links],4,NSEdgeInsetsMake(0,0,0,0));
        for(NSView *v in @[icon,body]) {v.translatesAutoresizingMaskIntoConstraints=NO;[alert addSubview:v];}
        [NSLayoutConstraint activateConstraints:@[
            [icon.leadingAnchor constraintEqualToAnchor:alert.leadingAnchor constant:10],[icon.topAnchor constraintEqualToAnchor:alert.topAnchor constant:10],
            [icon.widthAnchor constraintEqualToConstant:16],
            [body.leadingAnchor constraintEqualToAnchor:icon.trailingAnchor constant:8],[body.trailingAnchor constraintEqualToAnchor:alert.trailingAnchor constant:-10],
            [body.topAnchor constraintEqualToAnchor:alert.topAnchor constant:9],[body.bottomAnchor constraintEqualToAnchor:alert.bottomAnchor constant:-9],
            [detail.widthAnchor constraintEqualToAnchor:body.widthAnchor],
        ]];
        [alert setAccessibilityElement:YES];[alert setAccessibilityRole:NSAccessibilityGroupRole];[alert setAccessibilityLabel:[@"连不上后台，正在重连：" stringByAppendingString:text(self.data[@"connection"][@"error"])]];
        add(stack,alert);
    }
    // Four counts; status colour lives only on the dot and a zero stays quiet.
    NSDictionary *counts=self.data[@"counts"];
    NSView *grid=fill(color(p[@"sunken"]),radius);grid.layer.borderColor=border.CGColor;grid.layer.borderWidth=1;
    NSStackView *cells=line_of(@[],0);cells.distribution=NSStackViewDistributionFillEqually;cells.alignment=NSLayoutAttributeTop;
    NSArray *specs=@[@[@"waiting",@"等你回答",@"wait"],@[@"errors",@"出错",@"error"],@[@"review",@"待核对",@"review"],@[@"running",@"在跑",@"running"]];
    for(NSArray *spec in specs) {
        NSInteger n=[counts[spec[0]] integerValue];
        NSTextField *value=label([NSString stringWithFormat:@"%ld",(long)n],[t[@"count"] doubleValue],n?NSFontWeightSemibold:NSFontWeightRegular,n?ink:muted);
        value.font=[NSFont monospacedDigitSystemFontOfSize:[t[@"count"] doubleValue] weight:n?NSFontWeightSemibold:NSFontWeightRegular];
        NSStackView *cell=column_of(@[line_of(@[dot(color(p[spec[2]])),label(spec[1],chip,NSFontWeightRegular,secondary)],5),value],1,NSEdgeInsetsMake(7,10,8,10));
        [cell setAccessibilityElement:YES];[cell setAccessibilityLabel:[NSString stringWithFormat:@"%@ %ld",spec[1],(long)n]];
        if(cells.arrangedSubviews.count) {NSView *edge=fill(border,0);[cell addSubview:edge];[NSLayoutConstraint activateConstraints:@[[edge.leadingAnchor constraintEqualToAnchor:cell.leadingAnchor],[edge.topAnchor constraintEqualToAnchor:cell.topAnchor],[edge.bottomAnchor constraintEqualToAnchor:cell.bottomAnchor],[edge.widthAnchor constraintEqualToConstant:1]]];}
        [cells addArrangedSubview:cell];
    }
    [grid addSubview:cells];
    [NSLayoutConstraint activateConstraints:@[[cells.leadingAnchor constraintEqualToAnchor:grid.leadingAnchor],[cells.trailingAnchor constraintEqualToAnchor:grid.trailingAnchor],[cells.topAnchor constraintEqualToAnchor:grid.topAnchor],[cells.bottomAnchor constraintEqualToAnchor:grid.bottomAnchor]]];
    if(offline) grid.alphaValue=.55;
    add(stack,grid);
    NSArray *rows=self.data[@"rows"]?:@[];
    NSMutableArray<KLRow *> *built=[NSMutableArray new];
    if(rows.count) {
        NSStackView *section=spread(label(text(self.data[@"section"]),chip,NSFontWeightSemibold,muted),label(text(self.data[@"section_note"]),chip,NSFontWeightRegular,muted));
        section.edgeInsets=NSEdgeInsetsMake(0,4,0,4);add(stack,section);[stack setCustomSpacing:6 afterView:section];
        NSStackView *list=column_of(@[],0,NSEdgeInsetsMake(0,0,0,0));
        for(NSDictionary *row in rows) {KLRow *view=[self row:row tokens:t palette:p];add(list,view);[built addObject:view];}
        if(offline) list.alphaValue=.55;
        add(stack,list);
        NSInteger more=[self.data[@"more"] integerValue];
        if(more>0) {
            NSButton *rest=text_button([NSString stringWithFormat:@"还有 %ld 项",(long)more],secondary,small,NO,self,@selector(choose:),@"");
            [stack setCustomSpacing:8 afterView:list];
            NSStackView *holder=line_of(@[rest],0);holder.edgeInsets=NSEdgeInsetsMake(0,4,0,4);add(stack,holder);
        }
    } else if([self.data[@"empty"] isKindOfClass:NSDictionary.class]) {
        KLDashed *empty=[KLDashed new];empty.edge=color(p[@"border_strong"]);empty.translatesAutoresizingMaskIntoConstraints=NO;
        NSTextField *title=[NSTextField wrappingLabelWithString:text(self.data[@"empty"][@"title"])];title.font=[NSFont systemFontOfSize:[t[@"font"] doubleValue] weight:NSFontWeightSemibold];title.textColor=ink;
        NSStackView *body=column_of(@[title,label(text(self.data[@"empty"][@"note"]),small,NSFontWeightRegular,secondary)],4,NSEdgeInsetsMake(18,12,18,12));
        [empty addSubview:body];
        [NSLayoutConstraint activateConstraints:@[[body.leadingAnchor constraintEqualToAnchor:empty.leadingAnchor],[body.trailingAnchor constraintEqualToAnchor:empty.trailingAnchor],[body.topAnchor constraintEqualToAnchor:empty.topAnchor],[body.bottomAnchor constraintEqualToAnchor:empty.bottomAnchor],[title.widthAnchor constraintEqualToAnchor:body.widthAnchor constant:-24]]];
        add(stack,empty);
    }
    self.rows=built;
    for(NSUInteger i=0;i+1<built.count;i++) built[i].nextKeyView=built[i+1];
    // One line of foot: what is left behind on the left, where the rest lives on the right.
    NSView *rule=hairline(border);add(stack,rule);[stack setCustomSpacing:10 afterView:rule];
    NSMutableArray *left=[NSMutableArray new];
    if([self.data[@"foot_count"] isKindOfClass:NSNumber.class]) [left addObject:dot(color(p[@"stale"]))];
    [left addObject:label(text(self.data[@"foot"]),small,NSFontWeightRegular,secondary)];
    if([self.data[@"foot_count"] isKindOfClass:NSNumber.class]) {
        NSTextField *count=label([self.data[@"foot_count"] stringValue],small,NSFontWeightSemibold,secondary);
        count.font=[NSFont monospacedDigitSystemFontOfSize:small weight:NSFontWeightSemibold];[left addObject:count];
    }
    NSStackView *foot=spread(line_of(left,6),label(text(self.data[@"foot_note"]),meta,NSFontWeightRegular,muted));
    foot.alignment=NSLayoutAttributeCenterY;foot.edgeInsets=NSEdgeInsetsMake(0,4,0,4);add(stack,foot);
    // The view's one solid control.
    NSButton *open=[NSButton buttonWithTitle:@"打开工作台" target:self action:@selector(choose:)];open.identifier=@"";open.bordered=NO;
    open.wantsLayer=YES;open.layer.backgroundColor=color(p[@"accent"]).CGColor;open.layer.cornerRadius=radius;
    open.attributedTitle=[[NSAttributedString alloc]initWithString:open.title attributes:@{NSForegroundColorAttributeName:color(p[@"on_accent"]),NSFontAttributeName:[NSFont systemFontOfSize:[t[@"font"] doubleValue] weight:NSFontWeightSemibold]}];
    add(stack,open);[open.heightAnchor constraintEqualToConstant:[t[@"button"] doubleValue]].active=YES;
    if(built.count) built.lastObject.nextKeyView=open;
    // Fit the content (up to the design height): two rows should not leave half the panel blank.
    [stack.widthAnchor constraintEqualToConstant:width].active=YES;
    [vc.view layoutSubtreeIfNeeded];
    height=MIN(height,ceil(stack.fittingSize.height));
    [vc.view setFrameSize:NSMakeSize(width,height)];
    [self relight];
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
@implementation KLRoot
- (BOOL)acceptsFirstResponder { return YES; }
// Nothing is lit when the panel opens; ↓ lights the first row.
- (void)keyDown:(NSEvent *)event {
    if(event.keyCode==125) [self.owner focusRow:0 from:nil]; else [super keyDown:event];
}
@end
@implementation KLRow
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)canBecomeKeyView { return YES; }
- (BOOL)becomeFirstResponder { self.focused=YES;[self.owner relight];return YES; }
- (BOOL)resignFirstResponder { self.focused=NO;[self.owner relight];return YES; }
- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    for(NSTrackingArea *area in self.trackingAreas) [self removeTrackingArea:area];
    [self addTrackingArea:[[NSTrackingArea alloc]initWithRect:NSZeroRect options:NSTrackingMouseEnteredAndExited|NSTrackingActiveAlways|NSTrackingInVisibleRect owner:self userInfo:nil]];
}
- (void)mouseEntered:(NSEvent *)event { self.pointed=YES;[self.owner relight]; }
- (void)mouseExited:(NSEvent *)event { self.pointed=NO;[self.owner relight]; }
- (void)mouseDown:(NSEvent *)event {}
- (void)mouseUp:(NSEvent *)event {
    if(NSPointInRect([self convertPoint:event.locationInWindow fromView:nil],self.bounds)) [self.owner open:self.route];
}
- (void)keyDown:(NSEvent *)event {
    switch(event.keyCode) {
        case 36: case 49: case 76: [self.owner open:self.route]; break;
        case 125: [self.owner focusRow:1 from:self]; break;
        case 126: [self.owner focusRow:-1 from:self]; break;
        default: [super keyDown:event];
    }
}
- (BOOL)accessibilityPerformPress { [self.owner open:self.route];return YES; }
@end
void kl_desktop_install(KLCallback callback) {
    desktop=[KLDesktop new];desktop.callback=callback;
    desktop.item=[[NSStatusBar systemStatusBar]statusItemWithLength:NSVariableStatusItemLength];
    desktop.item.button.title=@"K";desktop.item.button.toolTip=@"Keepline";
    desktop.item.button.target=desktop;desktop.item.button.action=@selector(show:);
    desktop.popover=[NSPopover new];desktop.popover.behavior=NSPopoverBehaviorTransient;
    // An open panel repaints when the system switches between light and dark.
    [NSApp addObserver:desktop forKeyPath:@"effectiveAppearance" options:0 context:NULL];
    // Unbundled developer binaries cannot use UNUserNotificationCenter. The local .app can.
    if(NSBundle.mainBundle.bundleIdentifier) [UNUserNotificationCenter currentNotificationCenter].delegate=desktop;
}
void kl_desktop_update(const char *json) {
    NSData *bytes=[[NSString stringWithUTF8String:json]dataUsingEncoding:NSUTF8StringEncoding];
    NSError *error=nil;NSDictionary *data=[NSJSONSerialization JSONObjectWithData:bytes options:0 error:&error];
    if(!data) {[desktop route:[@"error:" stringByAppendingString:error.localizedDescription]];return;}
    desktop.data=data;desktop.item.button.title=data[@"badge"]?:@"K";
    BOOL offline=[data[@"connection"][@"error"] isKindOfClass:NSString.class];
    desktop.item.button.image=offline?offline_mark():nil;desktop.item.button.imagePosition=NSImageLeft;
    desktop.item.button.toolTip=offline?@"Keepline · 连不上后台":@"Keepline";
    if(desktop.popover.shown) [desktop rebuild];
}
void kl_notifications_enable(void) {
    if(!NSBundle.mainBundle.bundleIdentifier) {[desktop route:@"error:系统通知需要从 Keepline.app 启动"];return;}
    [[UNUserNotificationCenter currentNotificationCenter]requestAuthorizationWithOptions:UNAuthorizationOptionAlert|UNAuthorizationOptionSound completionHandler:^(BOOL granted,NSError *error){
        NSString *message=error?[@"error:" stringByAppendingString:error.localizedDescription]:(granted?@"notice:系统通知已允许":@"error:系统未允许通知，请在系统设置中手动打开 Keepline 通知；工作台列表仍可使用");
        dispatch_async(dispatch_get_main_queue(),^{[desktop route:message];});
    }];
}
/// The alert sound alone, for 提示音 without system notifications.
void kl_play_sound(void) {
    [[NSSound soundNamed:@"Glass"] play];
}
void kl_desktop_notify(const char *session,const char *title,const char *body,int sound) {
    if(!NSBundle.mainBundle.bundleIdentifier) {[desktop route:@"error:系统通知需要从 Keepline.app 启动"];return;}
    NSString *sid=[NSString stringWithUTF8String:session];
    UNMutableNotificationContent *content=[UNMutableNotificationContent new];content.title=[NSString stringWithUTF8String:title];content.body=[NSString stringWithUTF8String:body];content.userInfo=@{@"session":sid};
    if(sound) content.sound=[UNNotificationSound defaultSound];
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
// A terminal bell while Keepline is in the background: one Dock bounce, as in Terminal.app.
void kl_request_attention(void) { [NSApp requestUserAttention:NSInformationalRequest]; }
// The running app's Dock tile; the bundle's .icns still shows while it is not running.
void kl_set_dock_icon(const unsigned char *png, size_t length) {
    NSImage *image=[[NSImage alloc]initWithData:[NSData dataWithBytes:png length:length]];
    if(image) NSApp.applicationIconImage=image;
}

// Called only by the visual-test feature: draw the actual AppKit panel, not an HTML copy.
bool kl_desktop_snapshot(const char *path) {
    [desktop rebuild];
    NSView *view=desktop.popover.contentViewController.view;
    [view layoutSubtreeIfNeeded];
    NSBitmapImageRep *bitmap=[view bitmapImageRepForCachingDisplayInRect:view.bounds];
    [view cacheDisplayInRect:view.bounds toBitmapImageRep:bitmap];
    NSData *data=[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
    return [data writeToFile:[NSString stringWithUTF8String:path] atomically:YES];
}
