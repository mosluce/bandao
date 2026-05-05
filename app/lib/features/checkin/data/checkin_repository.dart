import 'package:dio/dio.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/api_client.dart';
import '../../../core/api/api_error.dart';
import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/checkin_status.dart';
import '../../../core/api/models/submit_checkin_event.dart';

class CheckinRepository {
  CheckinRepository(this._dio);

  final Dio _dio;

  Future<SubmitCheckinEventResponse> submit(
    SubmitCheckinEventRequest req,
  ) async {
    try {
      final res = await _dio.post<Map<String, dynamic>>(
        '/app/checkin/events',
        data: req.toJson(),
      );
      return SubmitCheckinEventResponse.fromJson(res.data!);
    } on DioException catch (e) {
      throw _unwrap(e);
    }
  }

  Future<CheckinUserStatusDto> status() async {
    try {
      final res = await _dio.get<Map<String, dynamic>>('/app/checkin/status');
      return CheckinUserStatusDto.fromJson(res.data!);
    } on DioException catch (e) {
      throw _unwrap(e);
    }
  }

  Future<List<CheckinEventDto>> events({String? before, int limit = 50}) async {
    try {
      final res = await _dio.get<List<dynamic>>(
        '/app/checkin/events',
        queryParameters: <String, dynamic>{
          'limit': limit,
          if (before != null) 'before': before,
        },
      );
      return (res.data ?? const <dynamic>[])
          .map((e) => CheckinEventDto.fromJson(e as Map<String, dynamic>))
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

final checkinRepositoryProvider =
    FutureProvider<CheckinRepository>((ref) async {
  final dio = await ref.watch(apiClientProvider.future);
  return CheckinRepository(dio);
});
