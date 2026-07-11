import java.util.Properties
import java.io.FileInputStream

plugins {
    id("com.android.application")
    id("kotlin-android")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
    // Google Services + Crashlytics — declared in settings.gradle.kts with
    // version + apply false; applied here so they bind to this module.
    id("com.google.gms.google-services")
    id("com.google.firebase.crashlytics")
}

// Production signing reads from android/key.properties (gitignored). When
// the file is absent (e.g. fresh clone, contributor without keystore) the
// release buildType falls back to debug signing so `flutter run --release`
// still works locally.
val keystoreProperties = Properties()
val keystorePropertiesFile = rootProject.file("key.properties")
if (keystorePropertiesFile.exists()) {
    keystoreProperties.load(FileInputStream(keystorePropertiesFile))
}

android {
    namespace = "tw.ccmos.app.bandao"
    compileSdk = flutter.compileSdkVersion
    // integration_test and jni (transitive, via workmanager 0.9.x) require
    // >= 28.2.13676358; pin explicitly so plugin upgrades don't surprise
    // local builds. NDK versions are backward compatible, so pinning the
    // highest required version satisfies every plugin.
    ndkVersion = "28.2.13676358"

    compileOptions {
        // AGP 8.11 requires JDK 17.
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = JavaVersion.VERSION_17.toString()
    }

    defaultConfig {
        applicationId = "tw.ccmos.app.bandao"
        minSdk = 24
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    signingConfigs {
        create("release") {
            keyAlias = keystoreProperties["keyAlias"] as String?
            keyPassword = keystoreProperties["keyPassword"] as String?
            storeFile = (keystoreProperties["storeFile"] as String?)?.let { file(it) }
            storePassword = keystoreProperties["storePassword"] as String?
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.getByName(
                if (keystorePropertiesFile.exists()) "release" else "debug"
            )
        }
    }
}

flutter {
    source = "../.."
}
