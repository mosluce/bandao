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
  String get appTitle => 'Argus';
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

  // Dev menu
  String get devMenuTitle => _isZh ? '伺服器' : 'Server';
  String get devMenuCurrentLabel =>
      _isZh ? '目前實際使用的網址' : 'Current effective URL';
  String get devMenuInputLabel =>
      _isZh ? '覆寫基礎網址' : 'Override base URL';
  String get devMenuSave => _isZh ? '儲存' : 'Save';
  String get devMenuClear => _isZh ? '清除' : 'Clear';
  String get devMenuApiPrefix => 'API';

  // Splash
  String get splashRetry => _isZh ? '重試' : 'Retry';
  String get splashLogout => _isZh ? '登出' : 'Sign out';
  String get splashNetworkMessage =>
      _isZh ? '無法連線伺服器，請稍後再試。' : 'Could not reach the server. Please retry.';

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
