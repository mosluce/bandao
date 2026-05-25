// Hand-rolled localization shim — see app_zh_TW.arb / app_en.arb for the
// source-of-truth strings. We do this instead of `flutter gen-l10n` because
// the v1 app ships zh_TW only and gen-l10n in Flutter 3.29 has friction
// with non-cwd project dirs. Adding a real locale should switch this back
// to ARB-driven codegen — see app/README.md.

import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';

class AppLocalizations {
  const AppLocalizations(this.locale);

  final Locale locale;

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  static AppLocalizations of(BuildContext context) {
    final loc = Localizations.of<AppLocalizations>(context, AppLocalizations);
    assert(loc != null, 'AppLocalizations.of called outside of MaterialApp');
    return loc!;
  }

  static const List<Locale> supportedLocales = <Locale>[
    Locale('zh', 'TW'),
    Locale('zh'),
    Locale('en'),
  ];

  // Login screen
  String get appTitle => '班到';
  String get loginTitle =>
      _isZh ? '登入' : 'Sign in';
  String get loginOrgCodeLabel =>
      _isZh ? '組織代碼' : 'Org code';
  String get loginUsernameLabel =>
      _isZh ? '帳號' : 'Username';
  String get loginPasswordLabel =>
      _isZh ? '密碼' : 'Password';
  String get loginSubmit =>
      _isZh ? '登入' : 'Sign in';

  // Errors
  String get errorInvalidCredentials =>
      _isZh ? '帳號、密碼或組織代碼錯誤' : 'Wrong account, password, or org code';
  String get errorInvalidPassword =>
      _isZh ? '目前密碼不正確' : 'Current password is incorrect';
  String get errorNetwork =>
      _isZh ? '連線失敗，請確認網路' : 'Connection failed. Check your network.';
  String get errorGeneric =>
      _isZh ? '發生錯誤，請稍後再試' : 'Something went wrong';
  String get errorLocationTrackingDisabled =>
      _isZh ? '組織已關閉定位追蹤' : 'Your organization has disabled location tracking';

  // Force change password
  String get forceChangeTitle =>
      _isZh ? '請變更密碼' : 'Change your password';
  String get forceChangeCurrentPasswordLabel =>
      _isZh ? '目前密碼' : 'Current password';
  String get forceChangeNewPasswordLabel =>
      _isZh ? '新密碼' : 'New password';
  String get forceChangeNewPasswordHint =>
      _isZh ? '至少 8 個字元' : 'At least 8 characters';
  String get forceChangeSubmit =>
      _isZh ? '變更密碼' : 'Change password';

  // Home
  String get homeLogout => _isZh ? '登出' : 'Sign out';
  String get homeStubTitle => _isZh ? '尚未實作' : 'Not implemented';
  String get homeStubSubtitle =>
      _isZh ? '打卡狀態將顯示於此。' : 'Checkin status will appear here.';

  // Logout confirm dialog (shown when queue holds non-empty rows for the
  // current user). Wipes happen on different-user login.
  String get logoutConfirmTitle =>
      _isZh ? '確定要登出嗎？' : 'Sign out?';
  String logoutConfirmBody(int n) => _isZh
      ? '你還有 $n 筆事件未處理。登出後若由其他帳號登入，這些事件會被清除。'
      : 'You have $n unsynced event(s). Signing out then signing in as a '
          'different account will erase them.';
  String get logoutConfirmCancel => _isZh ? '取消' : 'Cancel';
  String get logoutConfirmProceed => _isZh ? '仍要登出' : 'Sign out anyway';

  // Checkin event labels
  String get eventClockIn => _isZh ? '上班' : 'Clock in';
  String get eventClockOut => _isZh ? '下班' : 'Clock out';
  String get eventTransferOut => _isZh ? '轉出' : 'Transfer out';
  String get eventTransferIn => _isZh ? '轉入' : 'Transfer in';

  // Status pill
  String get statusOffDuty => _isZh ? '尚未上班' : 'Off duty';
  String get statusOnSite => _isZh ? '上班中' : 'On site';
  String get statusInTransit => _isZh ? '轉場中' : 'In transit';
  String elapsedShift(int hours, int minutes) => _isZh
      ? '已上班 $hours 時 $minutes 分'
      : 'On shift ${hours}h ${minutes}m';
  String get statusUnknownLocation => _isZh ? '位置確認中…' : 'Locating…';

  // Queue chip
  String get queueChipSending => _isZh ? '送出中' : 'Sending…';
  String queueChipPending(int n) =>
      _isZh ? '待送出 $n 筆' : 'Queued $n';
  String queueChipFailed(int n) =>
      _isZh ? '$n 筆失敗' : '$n failed';

  // Location permission
  String get locationBlockerMessage =>
      _isZh ? '需要定位權限才能打卡' : 'Location permission required to clock in';
  String get locationBlockerOpenSettings =>
      _isZh ? '開啟設定' : 'Open settings';
  String get locationUnavailable =>
      _isZh ? '無法取得位置，請確認 GPS 是否開啟' : 'Could not capture location.';

  // Location tracking consent dialog
  String get locationConsentTitle =>
      _isZh ? '啟用定位追蹤' : 'Enable location tracking';
  String get locationConsentBody => _isZh
      ? '上班期間會記錄您的位置，您可以在「我的工作日記」回顧自己今天的工作路線與走動距離；'
          '同時也會提供給組織管理員。在此功能下：'
      : 'While you are on shift the app records your location so you can review '
          'your own work-day in "My Work Day" — distance walked, route, totals. '
          'The same data is shared with your organization\'s admins. While active:';
  String get locationConsentBulletCadence =>
      _isZh ? '上班期間約每分鐘記錄一次位置' : 'Position is recorded roughly every minute';
  String get locationConsentBulletDistance =>
      _isZh ? '移動超過 100 公尺才會儲存' : 'Only saved when you have moved more than 100m';
  String get locationConsentBulletRetention =>
      _isZh ? '保存 90 天後自動清除' : 'Stored for 90 days, then automatically deleted';
  String get locationConsentBulletAudience => _isZh
      ? '您本人可於「我的工作日記」查閱，組織管理員亦可查閱'
      : 'Visible to you in "My Work Day", and to your organization\'s admins';
  String get locationConsentPrivacyLink =>
      _isZh ? '查看完整隱私政策' : 'View full privacy policy';
  String get locationConsentCancel => _isZh ? '取消' : 'Cancel';
  String get locationConsentProceed =>
      _isZh ? '同意並上班' : 'Agree & clock in';

  // Tracking chip on home
  String get trackingChipLabel => _isZh ? '定位追蹤中' : 'Tracking location';

  // Force-quit recovery banner
  String get trackingRecoveryBannerMessage =>
      _isZh ? '定位追蹤上次中斷過，已恢復記錄。' : 'Location tracking was interrupted; recording resumed.';
  String get trackingRecoveryBannerDismiss => _isZh ? '了解' : 'Got it';

  // Handover toast — template constructed in auth_provider; this getter
  // is the english fallback. Server-side string isn't localized here because
  // the message is built at notice-emit time. Kept for English builds.
  String handoverWipedNotice(int n) =>
      _isZh ? '前個帳號的 $n 筆未送事件已清除' : 'Cleared $n unsent events from previous account';

  // Onboarding tip (iOS background limitation)
  String get backgroundTipTitle =>
      _isZh ? '背景同步說明' : 'Background sync note';
  String get backgroundTipBody => _isZh
      ? 'iOS 會自行決定何時執行背景同步，請勿強制關閉「班到」，'
          '以免待送的打卡事件延遲上傳。'
      : 'iOS schedules background sync at its discretion. '
          'Do not force-quit Bandao while events are queued.';
  String get backgroundTipDismiss => _isZh ? '了解' : 'Got it';

  // History
  String get historyTitle => _isZh ? '事件歷史' : 'History';
  String get historyEntry => _isZh ? '事件歷史' : 'View history';
  String get historyEmpty =>
      _isZh ? '目前還沒有打卡事件' : 'No checkin events yet';
  String get historyLoadMore => _isZh ? '載入更多' : 'Load more';
  String get historyCopyDetails => _isZh ? '複製細節' : 'Copy details';
  String get historyDismiss => _isZh ? '關閉' : 'Dismiss';
  String get historyCopiedToast =>
      _isZh ? '細節已複製' : 'Details copied to clipboard';

  // History row badges
  String get badgePending => _isZh ? '待送出' : 'pending';
  String get badgeSending => _isZh ? '送出中' : 'sending';
  String get badgeFailed => _isZh ? '失敗' : 'failed';
  String get badgeSynced => _isZh ? '已上傳' : 'synced';

  // Error code friendly translations
  String friendlyErrorCode(String code, String fallbackMessage) {
    if (!_isZh) return fallbackMessage.isEmpty ? code : fallbackMessage;
    switch (code) {
      case 'INVALID_TRANSITION':
        return '狀態不允許此事件';
      case 'OUT_OF_ORDER':
        return '事件時間早於前次事件';
      case 'TRANSFER_DISABLED':
        return '組織已停用「轉場」功能';
      case 'NEEDS_PASSWORD_CHANGE':
        return '請先變更密碼';
      case 'UNAUTHORIZED':
        return '登入已失效';
      case 'NETWORK_ERROR':
        return '連線失敗';
      default:
        return fallbackMessage.isEmpty ? code : fallbackMessage;
    }
  }

  // Dev menu
  String get devMenuTitle => _isZh ? '伺服器' : 'Server';
  String get devMenuCurrentLabel =>
      _isZh ? '目前實際使用的網址' : 'Current effective URL';
  String get devMenuInputLabel =>
      _isZh ? '覆寫基礎網址' : 'Override base URL';
  String get devMenuSave => _isZh ? '儲存' : 'Save';
  String get devMenuClear => _isZh ? '清除' : 'Clear';
  String get devMenuSaved => _isZh ? '已儲存' : 'Saved';
  String get devMenuApiPrefix => 'API';
  String get devMenuPrivacyCurrentLabel =>
      _isZh ? '目前隱私政策網址' : 'Current privacy policy URL';
  String get devMenuPrivacyInputLabel =>
      _isZh ? '覆寫隱私政策網址' : 'Override privacy policy URL';

  // Splash
  String get splashRetry => _isZh ? '重試' : 'Retry';
  String get splashLogout => _isZh ? '登出' : 'Sign out';
  String get splashNetworkMessage =>
      _isZh ? '無法連線伺服器，請稍後再試。' : 'Could not reach the server. Please retry.';

  // Bottom navigation labels for the authenticated shell.
  String get navHome => _isZh ? '首頁' : 'Home';
  String get navHistory => _isZh ? '歷史' : 'History';

  // Trajectory ("我的工作日記") — the user-facing surface that justifies
  // UIBackgroundModes:location per App Review 2.5.4.
  String get trajectoryTitle => _isZh ? '我的工作日記' : 'My Work Day';
  String get trajectoryNavLabel => _isZh ? '我的軌跡' : 'My Trajectory';
  String get trajectoryEmpty => _isZh ? '該日無軌跡資料' : 'No trajectory for this day';
  String get trajectoryStatDistance => _isZh ? '走動距離' : 'Distance';
  String get trajectoryStatDuration => _isZh ? '在班時長' : 'On-shift';
  String get trajectoryStatPings => _isZh ? '位置點' : 'Pings';
  String get trajectoryTodayCardTitle => _isZh ? '我的今天' : 'Today';
  String get trajectoryTodayCardCta => _isZh ? '查看軌跡' : 'View trajectory';
  String get trajectoryPermissionTitle =>
      _isZh ? '尚未授權定位' : 'Location permission needed';
  String get trajectoryPermissionBody => _isZh
      ? '需要定位權限才能繪製您的工作軌跡。請至系統設定開啟。'
      : 'Allow location access in system settings to view your work-day trajectory.';
  String get trajectoryPermissionOpenSettings =>
      _isZh ? '前往系統設定' : 'Open settings';
  String get trajectoryAttribution => '© OpenStreetMap contributors © CARTO';
  String get trajectoryDateToday => _isZh ? '今天' : 'Today';
  String trajectoryDistanceKm(double km) =>
      '${km.toStringAsFixed(1)} ${_isZh ? '公里' : 'km'}';
  String trajectoryDurationHm(int hours, int minutes) {
    if (_isZh) {
      if (hours == 0) return '$minutes 分';
      return '$hours 小時 $minutes 分';
    }
    if (hours == 0) return '${minutes}m';
    return '${hours}h ${minutes}m';
  }

  bool get _isZh => locale.languageCode == 'zh';
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  bool isSupported(Locale locale) {
    return AppLocalizations.supportedLocales
        .any((s) => s.languageCode == locale.languageCode);
  }

  @override
  Future<AppLocalizations> load(Locale locale) async {
    return AppLocalizations(locale);
  }

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}
