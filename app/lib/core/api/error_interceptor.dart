import 'package:dio/dio.dart';

import 'api_error.dart';

/// Maps `DioException`s to [ApiException]. The rest of the app catches
/// `ApiException` only and never depends on dio types.
class ErrorInterceptor extends Interceptor {
  const ErrorInterceptor();

  @override
  void onError(DioException err, ErrorInterceptorHandler handler) {
    final apiErr = _translate(err);
    handler.reject(
      DioException(
        requestOptions: err.requestOptions,
        response: err.response,
        type: err.type,
        error: apiErr,
        stackTrace: err.stackTrace,
      ),
    );
  }

  ApiException _translate(DioException err) {
    // Network failures (no response). Includes connection timeouts and DNS
    // failures. Treat all of these as NETWORK_ERROR with status=0 so callers
    // can show a single retry-friendly message.
    if (err.response == null) {
      return ApiException.network(err.message ?? 'network error');
    }

    final status = err.response?.statusCode ?? 0;
    final body = err.response?.data;

    final parsed = _parseEnvelope(body);
    if (parsed != null) {
      return ApiException(
        status: status,
        code: parsed.code,
        message: parsed.message,
        retryAfter: parsed.retryAfter,
      );
    }

    // Unrecognised body shape — surface a best-effort generic exception so
    // upper layers can still match on the HTTP status.
    return ApiException(
      status: status,
      code: 'UNKNOWN',
      message: err.response?.statusMessage ?? err.message ?? 'unknown error',
    );
  }

  _ParsedError? _parseEnvelope(Object? body) {
    if (body is! Map<String, dynamic>) {
      return null;
    }
    final error = body['error'];
    if (error is! Map<String, dynamic>) {
      return null;
    }
    final code = error['code'];
    final message = error['message'];
    if (code is! String) {
      return null;
    }
    return _ParsedError(
      code: code,
      message: message is String ? message : '',
      retryAfter: error['retry_after'] is String
          ? error['retry_after'] as String
          : null,
    );
  }
}

class _ParsedError {
  const _ParsedError({
    required this.code,
    required this.message,
    this.retryAfter,
  });

  final String code;
  final String message;
  final String? retryAfter;
}
