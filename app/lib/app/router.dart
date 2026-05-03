import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../features/auth/presentation/dev_server_config_screen.dart';
import '../features/auth/presentation/force_password_change_screen.dart';
import '../features/auth/presentation/home_screen.dart';
import '../features/auth/presentation/login_screen.dart';
import '../features/auth/presentation/splash_screen.dart';
import '../features/auth/state/auth_provider.dart';
import '../features/auth/state/auth_state.dart';

/// Locked routes for v1.
class AppRoutes {
  const AppRoutes._();

  static const String splash = '/splash';
  static const String login = '/login';
  static const String forceChange = '/force-change-password';
  static const String home = '/';
  static const String devServerConfig = '/dev-server-config';
}

final routerProvider = Provider<GoRouter>((ref) {
  return GoRouter(
    initialLocation: AppRoutes.home,
    debugLogDiagnostics: kDebugMode,
    refreshListenable: _AuthRefreshNotifier(ref),
    redirect: (BuildContext context, GoRouterState state) {
      final authAsync = ref.read(authProvider);
      // Async-loading or rebuilding: park on splash. The redirect rules below
      // assume we have a concrete `AuthState` value.
      if (authAsync.isLoading) {
        return state.matchedLocation == AppRoutes.splash
            ? null
            : AppRoutes.splash;
      }
      final auth = authAsync.value;
      if (auth == null) {
        return state.matchedLocation == AppRoutes.splash
            ? null
            : AppRoutes.splash;
      }
      return _redirectFor(auth, state);
    },
    routes: <GoRoute>[
      GoRoute(
        path: AppRoutes.splash,
        builder: (BuildContext context, GoRouterState state) =>
            const SplashScreen(),
      ),
      GoRoute(
        path: AppRoutes.login,
        builder: (BuildContext context, GoRouterState state) =>
            const LoginScreen(),
      ),
      GoRoute(
        path: AppRoutes.forceChange,
        builder: (BuildContext context, GoRouterState state) =>
            const ForcePasswordChangeScreen(),
      ),
      GoRoute(
        path: AppRoutes.home,
        builder: (BuildContext context, GoRouterState state) =>
            const HomeScreen(),
      ),
      GoRoute(
        path: AppRoutes.devServerConfig,
        builder: (BuildContext context, GoRouterState state) {
          // Defensive: even if a release build navigates here, render an
          // inert "Not available" page. The login screen's tap handler is
          // the primary gate.
          if (kReleaseMode) {
            return const Scaffold(
              body: Center(child: Text('Not available')),
            );
          }
          return const DevServerConfigScreen();
        },
      ),
    ],
  );
});

String? _redirectFor(AuthState auth, GoRouterState state) {
  final loc = state.matchedLocation;
  switch (auth) {
    case AuthLoading():
      // Should be caught earlier; treat as splash.
      return loc == AppRoutes.splash ? null : AppRoutes.splash;
    case AuthUnauthenticated():
      // Allow /login and the dev menu; everything else (including splash) -> /login.
      if (loc == AppRoutes.login || loc == AppRoutes.devServerConfig) {
        return null;
      }
      return AppRoutes.login;
    case AuthAuthenticated(needsPasswordChange: true):
      if (loc == AppRoutes.forceChange) return null;
      return AppRoutes.forceChange;
    case AuthAuthenticated(needsPasswordChange: false):
      if (loc == AppRoutes.login || loc == AppRoutes.forceChange) {
        return AppRoutes.home;
      }
      return null;
    case AuthError():
      // Surface the failure on /login so the user can see the retry.
      if (loc == AppRoutes.login || loc == AppRoutes.devServerConfig) {
        return null;
      }
      return AppRoutes.login;
  }
}

/// Bridges `Riverpod` state changes to `GoRouter.refreshListenable`. Any
/// state change in `authProvider` notifies the router so `redirect` re-runs.
class _AuthRefreshNotifier extends ChangeNotifier {
  _AuthRefreshNotifier(this._ref) {
    _sub = _ref.listen<AsyncValue<AuthState>>(
      authProvider,
      (_, __) => notifyListeners(),
      fireImmediately: false,
    );
  }

  final Ref _ref;
  late final ProviderSubscription<AsyncValue<AuthState>> _sub;

  @override
  void dispose() {
    _sub.close();
    super.dispose();
  }
}
