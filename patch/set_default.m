// Didia Keeper helper: make an app the default web browser (http, https and HTML files),
// or with --query print the app that opens web links now. macOS asks the person to confirm.
#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

static void printDefault(void) {
    NSURL *app = [NSWorkspace.sharedWorkspace URLForApplicationToOpenURL:[NSURL URLWithString:@"https://example.com"]];
    printf("default: %s\n", app.path.UTF8String ?: "");
}

int main(int argc, char **argv) {
    @autoreleasepool {
        if (argc < 2) { fprintf(stderr, "usage: set-default <app path> | --query\n"); return 2; }
        if (!strcmp(argv[1], "--query")) { printDefault(); return 0; }
        NSURL *app = [NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[1]]];
        __block int pending = 3, failures = 0;
        void (^done)(NSError *) = ^(NSError *e) {
            if (e) { failures++; fprintf(stderr, "%s\n", e.localizedDescription.UTF8String); }
            pending--;
        };
        NSWorkspace *ws = NSWorkspace.sharedWorkspace;
        [ws setDefaultApplicationAtURL:app toOpenURLsWithScheme:@"http" completionHandler:done];
        [ws setDefaultApplicationAtURL:app toOpenURLsWithScheme:@"https" completionHandler:done];
        [ws setDefaultApplicationAtURL:app toOpenContentType:UTTypeHTML completionHandler:done];
        NSDate *until = [NSDate dateWithTimeIntervalSinceNow:180]; // the person confirms a system dialog
        while (pending > 0 && [until timeIntervalSinceNow] > 0)
            [NSRunLoop.currentRunLoop runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.2]];
        printDefault();
        return pending > 0 ? 3 : (failures ? 1 : 0);
    }
}
