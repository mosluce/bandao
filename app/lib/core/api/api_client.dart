import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../storage/api_base_url.dart';
import '../storage/secure_storage.dart';
import 'auth_interceptor.dart';
import 'error_interceptor.dart';
import 'log_interceptor.dart';

/// Async Riverpod provider that builds a `Dio` instance with the resolved
/// base URL, sane timeouts, and the three interceptors registered in order:
/// log (debug only) -> auth -> error.
///
/// Rebuilds when `effectiveBaseUrlProvider` is invalidated (e.g. after the
/// dev menu saves a new override).
final apiClientProvider = FutureProvider<Dio>((ref) async {
  final baseUrl = await ref.watch(effectiveBaseUrlProvider.future);
  final storage = ref.watch(secureStorageProvider);

  final dio = Dio(
    BaseOptions(
      baseUrl: baseUrl,
      connectTimeout: const Duration(seconds: 10),
      receiveTimeout: const Duration(seconds: 15),
      sendTimeout: const Duration(seconds: 15),
      contentType: 'application/json',
      responseType: ResponseType.json,
    ),
  );

  if (kDebugMode) {
    dio.interceptors.add(AppLogInterceptor());
  }
  dio.interceptors.add(AuthInterceptor(storage));
  dio.interceptors.add(const ErrorInterceptor());

  return dio;
});
