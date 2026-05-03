import 'package:dio/dio.dart' as dio;
import 'package:logger/logger.dart';

/// Pretty-prints requests / responses / errors. Wrapped via `kDebugMode` at
/// registration time so the import is harmless in release.
class AppLogInterceptor extends dio.Interceptor {
  AppLogInterceptor([Logger? logger])
      : _logger = logger ?? Logger(printer: PrettyPrinter(methodCount: 0));

  final Logger _logger;

  @override
  void onRequest(
    dio.RequestOptions options,
    dio.RequestInterceptorHandler handler,
  ) {
    _logger.d('-> ${options.method} ${options.uri}');
    handler.next(options);
  }

  @override
  void onResponse(
    dio.Response<dynamic> response,
    dio.ResponseInterceptorHandler handler,
  ) {
    _logger.d(
      '<- ${response.statusCode} ${response.requestOptions.method} '
      '${response.requestOptions.uri}',
    );
    handler.next(response);
  }

  @override
  void onError(
    dio.DioException err,
    dio.ErrorInterceptorHandler handler,
  ) {
    _logger.w(
      '!! ${err.requestOptions.method} ${err.requestOptions.uri} '
      '-> ${err.response?.statusCode ?? 'no-response'} '
      '(${err.type.name})',
    );
    handler.next(err);
  }
}
