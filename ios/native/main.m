#import <UIKit/UIKit.h>
#import <WebKit/WebKit.h>

@interface VC : UIViewController
@property (strong) WKWebView *web;
@end
@implementation VC
- (UIStatusBarStyle)preferredStatusBarStyle { return UIStatusBarStyleLightContent; }
- (void)viewDidLoad {
  [super viewDidLoad];
  UIColor *bg = [UIColor colorWithRed:6/255.0 green:11/255.0 blue:22/255.0 alpha:1];
  self.view.backgroundColor = bg;
  self.web = [[WKWebView alloc] initWithFrame:CGRectZero configuration:[WKWebViewConfiguration new]];
  self.web.opaque = NO; self.web.backgroundColor = bg; self.web.scrollView.backgroundColor = bg;
  self.web.scrollView.contentInsetAdjustmentBehavior = UIScrollViewContentInsetAdjustmentNever;
  self.web.allowsBackForwardNavigationGestures = YES;
  self.web.translatesAutoresizingMaskIntoConstraints = NO;
  [self.view addSubview:self.web];
  UILayoutGuide *g = self.view.safeAreaLayoutGuide;
  [NSLayoutConstraint activateConstraints:@[
    [self.web.leadingAnchor constraintEqualToAnchor:self.view.leadingAnchor],
    [self.web.trailingAnchor constraintEqualToAnchor:self.view.trailingAnchor],
    [self.web.topAnchor constraintEqualToAnchor:g.topAnchor],
    [self.web.bottomAnchor constraintEqualToAnchor:self.view.bottomAnchor]]];
  NSURL *www = [[NSBundle mainBundle] URLForResource:@"www" withExtension:nil];
  [self.web loadFileURL:[www URLByAppendingPathComponent:@"index.html"] allowingReadAccessToURL:www];
}
@end

@interface AD : UIResponder <UIApplicationDelegate>
@property (nonatomic, strong) UIWindow *window;
@end
@implementation AD
- (BOOL)application:(UIApplication *)a didFinishLaunchingWithOptions:(NSDictionary *)o {
  self.window = [[UIWindow alloc] initWithFrame:UIScreen.mainScreen.bounds];
  self.window.rootViewController = [VC new];
  [self.window makeKeyAndVisible];
  return YES;
}
@end

int main(int argc, char *argv[]) {
  @autoreleasepool { return UIApplicationMain(argc, argv, nil, NSStringFromClass([AD class])); }
}
