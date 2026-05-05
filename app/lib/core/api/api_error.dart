import 'package:flutter/widgets.dart';

import '../../l10n/app_localizations.dart';

/// Stable codes used by the api error envelope. Mirrors `api/src/error.rs`.
class ApiErrorCode {
  const ApiErrorCode._();

  static const String invalidCredentials = 'INVALID_CREDENTIALS';
  static const String invalidPassword = 'INVALID_PASSWORD';
  static const String needsPasswordChange = 'NEEDS_PASSWORD_CHANGE';
  static const String unauthorized = 'UNAUTHORIZED';
  static const String forbidden = 'FORBIDDEN';
  static const String validation = 'VALIDATION';
  static const String network = 'NETWORK_ERROR';
  static const String locationTrackingDisabled = 'LOCATION_TRACKING_DISABLED';
}

/// Normalized exception thrown out of the dio error interceptor. The rest of
/// the app catches `ApiException`, never `DioException` directly.
class ApiException implements Exception {
  const ApiException({
    required this.status,
    required this.code,
    required this.message,
    this.retryAfter,
  });

  /// 401 + INVALID_CREDENTIALS — login form rejection.
  factory ApiException.invalidCredentials([String? message]) => ApiException(
        status: 401,
        code: ApiErrorCode.invalidCredentials,
        message: message ?? 'invalid credentials',
      );

  /// 401 + INVALID_PASSWORD — wrong current password on /me/password.
  factory ApiException.invalidPassword([String? message]) => ApiException(
        status: 401,
        code: ApiErrorCode.invalidPassword,
        message: message ?? 'invalid password',
      );

  /// 423 + NEEDS_PASSWORD_CHANGE — first-login gate.
  factory ApiException.needsPasswordChange([String? message]) => ApiException(
        status: 423,
        code: ApiErrorCode.needsPasswordChange,
        message: message ?? 'password change required',
      );

  /// 401 + UNAUTHORIZED — generic missing/expired session.
  factory ApiException.unauthorized([String? message]) => ApiException(
        status: 401,
        code: ApiErrorCode.unauthorized,
        message: message ?? 'unauthorized',
      );

  /// 403 + FORBIDDEN.
  factory ApiException.forbidden([String? message]) => ApiException(
        status: 403,
        code: ApiErrorCode.forbidden,
        message: message ?? 'forbidden',
      );

  /// 400 + VALIDATION — server-side validation failure.
  factory ApiException.validation([String? message]) => ApiException(
        status: 400,
        code: ApiErrorCode.validation,
        message: message ?? 'validation failed',
      );

  /// status=0 + NETWORK_ERROR — no response from the server.
  factory ApiException.network([String? message]) => ApiException(
        status: 0,
        code: ApiErrorCode.network,
        message: message ?? 'network error',
      );

  final int status;
  final String code;
  final String message;

  /// Optional Retry-After hint propagated from the response headers / body.
  final String? retryAfter;

  @override
  String toString() =>
      'ApiException(status: $status, code: $code, message: $message)';
}

/// UI-side helper: translate an [ApiException] into the friendly Chinese
/// string the design.md spec calls out. Falls back to the API's `message`
/// for unknown codes.
extension ApiExceptionFriendly on ApiException {
  String friendlyZh(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    switch (code) {
      case ApiErrorCode.invalidCredentials:
        return l10n.errorInvalidCredentials;
      case ApiErrorCode.invalidPassword:
        return l10n.errorInvalidPassword;
      case ApiErrorCode.network:
        return l10n.errorNetwork;
      case ApiErrorCode.validation:
        // Server-supplied message is most informative for validation errors.
        return message.isEmpty ? l10n.errorGeneric : message;
      case ApiErrorCode.locationTrackingDisabled:
        return l10n.errorLocationTrackingDisabled;
      default:
        return message.isEmpty ? l10n.errorGeneric : message;
    }
  }
}
