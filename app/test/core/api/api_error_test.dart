import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/core/api/api_error.dart';
import 'package:argus_app/core/api/error_interceptor.dart';

void main() {
  group('ErrorInterceptor parses envelope', () {
    test('401 INVALID_CREDENTIALS', () {
      final got = _runInterceptor(
        DioException(
          requestOptions: RequestOptions(path: '/app/auth/login'),
          response: Response<Map<String, dynamic>>(
            requestOptions: RequestOptions(path: '/app/auth/login'),
            statusCode: 401,
            data: <String, dynamic>{
              'error': <String, dynamic>{
                'code': 'INVALID_CREDENTIALS',
                'message': 'wrong creds',
              },
            },
          ),
        ),
      );
      expect(got.status, 401);
      expect(got.code, 'INVALID_CREDENTIALS');
      expect(got.message, 'wrong creds');
      expect(got.retryAfter, isNull);
    });

    test('401 UNAUTHORIZED', () {
      final got = _runInterceptor(
        _buildDioErr(401, 'UNAUTHORIZED', 'unauthorized'),
      );
      expect(got.status, 401);
      expect(got.code, 'UNAUTHORIZED');
      expect(got.message, 'unauthorized');
    });

    test('423 NEEDS_PASSWORD_CHANGE', () {
      final got = _runInterceptor(
        _buildDioErr(423, 'NEEDS_PASSWORD_CHANGE', 'change required'),
      );
      expect(got.status, 423);
      expect(got.code, 'NEEDS_PASSWORD_CHANGE');
    });

    test('400 VALIDATION', () {
      final got = _runInterceptor(
        _buildDioErr(400, 'VALIDATION', 'bad password'),
      );
      expect(got.status, 400);
      expect(got.code, 'VALIDATION');
      expect(got.message, 'bad password');
    });

    test('429 with retry_after', () {
      final got = _runInterceptor(
        DioException(
          requestOptions: RequestOptions(path: '/foo'),
          response: Response<Map<String, dynamic>>(
            requestOptions: RequestOptions(path: '/foo'),
            statusCode: 429,
            data: <String, dynamic>{
              'error': <String, dynamic>{
                'code': 'RATE_LIMITED',
                'message': 'slow down',
                'retry_after': '60',
              },
            },
          ),
        ),
      );
      expect(got.code, 'RATE_LIMITED');
      expect(got.retryAfter, '60');
    });

    test('network error -> status=0 NETWORK_ERROR', () {
      final got = _runInterceptor(
        DioException(
          requestOptions: RequestOptions(path: '/app/me'),
          type: DioExceptionType.connectionError,
          message: 'connection refused',
        ),
      );
      expect(got.status, 0);
      expect(got.code, ApiErrorCode.network);
      expect(got.message, contains('connection refused'));
    });

    test('unknown body shape falls back to UNKNOWN', () {
      final got = _runInterceptor(
        DioException(
          requestOptions: RequestOptions(path: '/foo'),
          response: Response<Object>(
            requestOptions: RequestOptions(path: '/foo'),
            statusCode: 500,
            statusMessage: 'internal',
            data: 'not a json envelope',
          ),
        ),
      );
      expect(got.status, 500);
      expect(got.code, 'UNKNOWN');
    });
  });

  group('ApiException factories', () {
    test('invalidCredentials', () {
      final e = ApiException.invalidCredentials();
      expect(e.status, 401);
      expect(e.code, ApiErrorCode.invalidCredentials);
    });

    test('needsPasswordChange', () {
      final e = ApiException.needsPasswordChange();
      expect(e.status, 423);
    });

    test('network has status 0', () {
      final e = ApiException.network();
      expect(e.status, 0);
      expect(e.code, ApiErrorCode.network);
    });
  });
}

DioException _buildDioErr(int code, String errCode, String message) {
  return DioException(
    requestOptions: RequestOptions(path: '/foo'),
    response: Response<Map<String, dynamic>>(
      requestOptions: RequestOptions(path: '/foo'),
      statusCode: code,
      data: <String, dynamic>{
        'error': <String, dynamic>{'code': errCode, 'message': message},
      },
    ),
  );
}

/// Runs the interceptor's onError synchronously and pulls the resulting
/// `ApiException` out of the rejected handler.
ApiException _runInterceptor(DioException err) {
  const interceptor = ErrorInterceptor();
  ApiException? captured;
  final handler = _CapturingHandler((rejected) {
    final inner = rejected.error;
    if (inner is ApiException) {
      captured = inner;
    }
  });
  interceptor.onError(err, handler);
  expect(captured, isNotNull, reason: 'handler.reject was not called');
  return captured!;
}

class _CapturingHandler extends ErrorInterceptorHandler {
  _CapturingHandler(this._onReject);

  final void Function(DioException) _onReject;

  @override
  void reject(DioException error, [bool callFollowingErrorInterceptor = true]) {
    _onReject(error);
  }
}
