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
    // flutter_secure_storage and path_provider_android require >= 27.0.12077973;
    // pin explicitly so plugin upgrades don't surprise local builds.
    ndkVersion = "27.0.12077973"

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    kotlinOptions {
        jvmTarget = JavaVersion.VERSION_11.toString()
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
