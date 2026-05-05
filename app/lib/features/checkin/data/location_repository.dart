import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/api_client.dart';
import '../../../core/api/api_error.dart';
import '../../../core/api/models/location_ping.dart';

/// Thin wrapper around `POST /app/checkin/locations`. Throws `ApiException`
/// on transport / 4xx / 5xx errors via the existing `dio` interceptor.
class LocationRepository {
  LocationRepository(this._dio);

  final Dio _dio;

  Future<SubmitLocationPingsResponse> submitBatch(
    SubmitLocationPingsRequest req,
  ) async {
    try {
      final res = await _dio.post<Map<String, dynamic>>(
        '/app/checkin/locations',
        data: req.toJson(),
      );
      return SubmitLocationPingsResponse.fromJson(res.data!);
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

final locationRepositoryProvider =
    FutureProvider<LocationRepository>((ref) async {
  final dio = await ref.watch(apiClientProvider.future);
  return LocationRepository(dio);
});
