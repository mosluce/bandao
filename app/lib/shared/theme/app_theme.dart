import 'package:flutter/material.dart';

/// M3 light-only theme for v1. Dark mode is a follow-up.
class AppTheme {
  const AppTheme._();

  /// Brand seed colour. Provisional — final palette ships with the rename.
  static const Color seed = Color(0xFF1F4E5F);

  static ThemeData light() {
    final scheme = ColorScheme.fromSeed(
      seedColor: seed,
      brightness: Brightness.light,
    );
    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      visualDensity: VisualDensity.adaptivePlatformDensity,
      appBarTheme: AppBarTheme(
        backgroundColor: scheme.surface,
        foregroundColor: scheme.onSurface,
        elevation: 0,
        centerTitle: false,
      ),
    );
  }
}
