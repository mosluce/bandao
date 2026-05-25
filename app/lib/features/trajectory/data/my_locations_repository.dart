import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/api_client.dart';
import '../../../core/api/api_error.dart';
import '../../../core/api/models/location_ping.dart';

/// Thin wrapper around `GET /app/checkin/me/locations`. The endpoint
/// returns the caller's own pings (identity from bearer token); the toggle
/// is NOT consulted server-side so a refetch after the org turns tracking
/// off still resolves with the pings already on file.
class MyLocationsRepository {
  MyLocationsRepository(this._dio);

  final Dio _dio;

  Future<List<LocationPingDto>> listForRange({
    required DateTime from,
    required DateTime to,
    int? limit,
  }) async {
    try {
      final res = await _dio.get<List<dynamic>>(
        '/app/checkin/me/locations',
        queryParameters: <String, dynamic>{
          'from': from.toUtc().toIso8601String(),
          'to': to.toUtc().toIso8601String(),
          if (limit != null) 'limit': limit,
        },
      );
      final raw = res.data ?? const <dynamic>[];
      return raw
          .map((e) => LocationPingDto.fromJson(e as Map<String, dynamic>))
          .toList(growable: false);
    } on DioException catch (e) {
      throw _unwrap(e);
    }
  }

  ApiException _unwrap(DioException e) {
    final err = e.error;
    if (err is ApiException) return err;
    return ApiException.network(e.message ?? 'network error');
  }
}

final myLocationsRepositoryProvider =
    FutureProvider<MyLocationsRepository>((ref) async {
  final dio = await ref.watch(apiClientProvider.future);
  return MyLocationsRepository(dio);
});
