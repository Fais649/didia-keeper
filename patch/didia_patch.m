// Didia patch: loaded into a re-signed copy of the official Dia browser.
// It reads Contents/Resources/DidiaPatch.plist (written by Didia Keeper) and
//  1. registers Dia's own feature-flag overrides (UserDefaults key
//     "transientFeatureFlagOverrides", format "flag=value|flag=value"),
//     which switches the AI and Atlassian features off at the source;
//  2. registers a few plain defaults (e.g. page-content extraction off);
//  3. as a backup, hides leftover buttons and menu items by label or class;
//  4. optionally keeps "--enable-applescript-javascript" on;
#import <Cocoa/Cocoa.h>
#import <mach-o/dyld.h>
#include <unistd.h>
#include <dlfcn.h>

static NSString *logPath;
static NSMutableSet *logged;
static NSDictionary *config;
static NSArray *hideExact, *hidePrefixes, *hideContains, *hideClasses, *hideMenuTitles;

static void logLine(NSString *s) {
    if (!logPath) return;
    FILE *f = fopen(logPath.UTF8String, "a");
    if (f) { fprintf(f, "%s\n", s.UTF8String); fclose(f); }
}

static NSArray *arrayFor(NSString *key) {
    id v = config[key];
    return [v isKindOfClass:NSArray.class] ? v : @[];
}

static NSDictionary *loadConfig(void) {
    char path[4096]; uint32_t size = sizeof(path);
    if (_NSGetExecutablePath(path, &size) != 0) return @{};
    NSString *exe = [[NSString stringWithUTF8String:path] stringByResolvingSymlinksInPath];
    NSString *plist = [[[exe stringByDeletingLastPathComponent] stringByDeletingLastPathComponent]
        stringByAppendingPathComponent:@"Resources/DidiaPatch.plist"];
    NSDictionary *d = [NSDictionary dictionaryWithContentsOfFile:plist];
    return d ?: @{};
}

static NSString *labelOf(NSView *v) {
    NSString *l = nil;
    @try { l = [v accessibilityLabel]; } @catch (id e) {}
    if (!l.length && [v respondsToSelector:@selector(title)]) {
        @try { id t = [(id)v title]; if ([t isKindOfClass:NSString.class]) l = t; } @catch (id e) {}
    }
    return l;
}

static BOOL labelMatches(NSString *label) {
    if (!label.length) return NO;
    for (NSString *s in hideExact) if ([label isEqualToString:s]) return YES;
    for (NSString *s in hidePrefixes) if ([label hasPrefix:s]) return YES;
    for (NSString *s in hideContains)
        if ([label rangeOfString:s options:NSCaseInsensitiveSearch].location != NSNotFound) return YES;
    return NO;
}

static BOOL classMatches(NSString *cls) {
    for (NSString *s in hideClasses) if ([cls rangeOfString:s].location != NSNotFound) return YES;
    return NO;
}

static void walk(NSView *v) {
    NSString *cls = NSStringFromClass(v.class);
    if ([cls hasPrefix:@"RenderWidgetHostView"] || [cls hasPrefix:@"WebContents"]) return; // page content
    NSString *label = labelOf(v);
    if (!v.hidden && (classMatches(cls) || labelMatches(label))) {
        v.hidden = YES;
        NSString *key = [NSString stringWithFormat:@"%@|%@", cls, label ?: @""];
        if (![logged containsObject:key]) {
            [logged addObject:key];
            logLine([NSString stringWithFormat:@"hid %@ (%@)", cls, label ?: @""]);
        }
        return;
    }
    for (NSView *s in v.subviews) walk(s);
}

static void sweepMenu(NSMenu *menu) {
    for (NSMenuItem *item in menu.itemArray) {
        if (!item.hidden && [hideMenuTitles containsObject:item.title]) {
            item.hidden = YES;
            logLine([NSString stringWithFormat:@"hid menu item %@", item.title]);
        }
        if (item.submenu) sweepMenu(item.submenu);
    }
}

static void sweep(void) {
    for (NSWindow *w in NSApp.windows) {
        NSView *root = w.contentView.superview ?: w.contentView;
        if (root) walk(root);
    }
    if (hideMenuTitles.count && NSApp.mainMenu) sweepMenu(NSApp.mainMenu);
}


static void ensureAppleScriptJavaScript(int argc, char **argv) {
    static const char *flag = "--enable-applescript-javascript";
    for (int i = 1; i < argc; i++)
        if (!strcmp(argv[i], flag) || !strncmp(argv[i], "--type=", 7)) return;
    char path[4096]; uint32_t size = sizeof(path);
    if (_NSGetExecutablePath(path, &size) != 0) return;
    char **next = calloc(argc + 2, sizeof(char *));
    if (!next) return;
    for (int i = 0; i < argc; i++) next[i] = argv[i];
    next[argc] = (char *)flag;
    execv(path, next); // returns only on failure: keep starting without the flag
    free(next);
}

__attribute__((constructor)) static void init(int argc, char **argv) {
    @autoreleasepool {
        config = loadConfig();
        if ([config[@"appleScriptJavaScript"] boolValue]) ensureAppleScriptJavaScript(argc, argv);
        for (int i = 1; i < argc; i++) if (!strncmp(argv[i], "--type=", 7)) return; // helper process

        logPath = [NSHomeDirectory() stringByAppendingPathComponent:@"Library/Logs/Didia.log"];
        logged = [NSMutableSet set];
        hideExact = arrayFor(@"hideLabels");
        hidePrefixes = arrayFor(@"hideLabelPrefixes");
        hideContains = arrayFor(@"hideLabelContaining");
        hideClasses = arrayFor(@"hideViewClasses");
        hideMenuTitles = arrayFor(@"hideMenuItems");

        NSMutableDictionary *reg = [NSMutableDictionary dictionary];
        NSDictionary *defaults = config[@"defaults"];
        if ([defaults isKindOfClass:NSDictionary.class]) [reg addEntriesFromDictionary:defaults];
        NSString *flags = config[@"flagOverrides"];
        if ([flags isKindOfClass:NSString.class] && flags.length) reg[@"transientFeatureFlagOverrides"] = flags;
        NSUserDefaults *ud = [NSUserDefaults standardUserDefaults];
        [ud registerDefaults:reg];
        // Also put them in the launch-argument domain, which outranks saved preferences.
        @try {
            NSMutableDictionary *arg = [[ud volatileDomainForName:NSArgumentDomain] mutableCopy] ?: [NSMutableDictionary dictionary];
            [arg addEntriesFromDictionary:reg];
            [ud removeVolatileDomainForName:NSArgumentDomain];
            [ud setVolatileDomain:arg forName:NSArgumentDomain];
        } @catch (id e) {
            logLine([NSString stringWithFormat:@"couldn't raise defaults priority: %@", e]);
        }

        NSUInteger flagCount = flags.length ? [flags componentsSeparatedByString:@"|"].count : 0;
        logLine([NSString stringWithFormat:@"--- Didia patch loaded pid %d, %lu flags off, %lu defaults",
                 getpid(), (unsigned long)flagCount, (unsigned long)defaults.count]);
        if (hideExact.count + hidePrefixes.count + hideContains.count + hideClasses.count + hideMenuTitles.count) {
            dispatch_async(dispatch_get_main_queue(), ^{
                [NSTimer scheduledTimerWithTimeInterval:0.5 repeats:YES block:^(NSTimer *t) { sweep(); }];
            });
        }
    }
}
