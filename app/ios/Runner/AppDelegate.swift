import Flutter
import UIKit
import workmanager_apple

@main
@objc class AppDelegate: FlutterAppDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    // Register the BGProcessingTask identifier with workmanager BEFORE
    // GeneratedPluginRegistrant runs and BEFORE super.application — otherwise
    // any later `BGTaskScheduler.submitTaskRequest` for this id crashes the
    // app at native level.
    WorkmanagerPlugin.registerBGProcessingTask(withIdentifier: "tw.ccmos.app.bandao.queue-drain")

    GeneratedPluginRegistrant.register(with: self)
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }
}
