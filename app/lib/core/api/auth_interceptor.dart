import 'package:dio/dio.dart';

import '../storage/secure_storage.dart';

/// Injects `Authorization: Bearer <token>` on outbound `/app/*` requests
/// when a token is present in secure storage. Requests outside `/app/*` are
/// untouched even if a token is stored, so the interceptor can sit on a
/// shared dio instance.
class AuthInterceptor extends Interceptor {
  AuthInterceptor(this._storage);

  final SecureStorage _storage;

  @override
  Future<void> onRequest(
    RequestOptions options,
    RequestInterceptorHandler handler,
  ) async {
    if (!_isAppPath(options.path)) {
      handler.next(options);
      return;
    }
    final token = await _storage.readToken();
    if (token != null && token.isNotEmpty) {
      options.headers['Authorization'] = 'Bearer $token';
    }
    handler.next(options);
  }

  bool _isAppPath(String path) {
    // `path` may be relative ("/app/me") or absolute
    // ("https://api.example.com/app/me"). Match either.
    final uri = Uri.tryParse(path);
    final segment = uri?.path ?? path;
    return segment.startsWith('/app/');
  }
}
