import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/api_client.dart';
import '../../../core/api/api_error.dart';
import '../../../core/api/models/auth_responses.dart';

/// Thin wrapper around the dio client for the four auth + identity calls.
/// Each method throws `ApiException` on errors; callers never depend on
/// dio types.
class AuthRepository {
  AuthRepository(this._dio);

  final Dio _dio;

  /// `POST /app/auth/login` — public.
  Future<LoginResponse> login({
    required String orgCode,
    required String username,
    required String password,
  }) async {
    try {
      final res = await _dio.post<Map<String, dynamic>>(
        '/app/auth/login',
        data: <String, dynamic>{
          'org_code': orgCode,
          'username': username,
          'password': password,
        },
      );
      return LoginResponse.fromJson(res.data!);
    } on DioException catch (e) {
      throw _unwrap(e);
    }
  }

  /// `POST /app/auth/logout` — best-effort. The notifier swallows errors.
  Future<void> logout() async {
    try {
      await _dio.post<void>('/app/auth/logout');
    } on DioException catch (e) {
      throw _unwrap(e);
    }
  }

  /// `GET /app/me` — Bearer auth required.
  Future<MeResponse> me() async {
    try {
      final res = await _dio.get<Map<String, dynamic>>('/app/me');
      return MeResponse.fromJson(res.data!);
    } on DioException catch (e) {
      throw _unwrap(e);
    }
  }

  /// `POST /app/me/password` — Bearer auth required.
  Future<void> changePassword({
    required String currentPassword,
    required String newPassword,
  }) async {
    try {
      await _dio.post<void>(
        '/app/me/password',
        data: <String, dynamic>{
          'current_password': currentPassword,
          'new_password': newPassword,
        },
      );
    } on DioException catch (e) {
      throw _unwrap(e);
    }
  }

  /// `ErrorInterceptor` stuffs an `ApiException` into `DioException.error`.
  /// Pull it out here so callers see the friendly type. Network errors with
  /// no interceptor coverage fall back to `ApiException.network`.
  ApiException _unwrap(DioException e) {
    final err = e.error;
    if (err is ApiException) {
      return err;
    }
    return ApiException.network(e.message ?? 'network error');
  }
}

final authRepositoryProvider = FutureProvider<AuthRepository>((ref) async {
  final dio = await ref.watch(apiClientProvider.future);
  return AuthRepository(dio);
});
