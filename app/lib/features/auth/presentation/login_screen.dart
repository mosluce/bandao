import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../../app/router.dart';
import '../../../core/api/api_error.dart';
import '../../../core/storage/api_base_url.dart';
import '../../../core/storage/secure_storage.dart';
import '../../../l10n/app_localizations.dart';
import '../state/auth_provider.dart';

class LoginScreen extends ConsumerStatefulWidget {
  const LoginScreen({super.key});

  @override
  ConsumerState<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends ConsumerState<LoginScreen> {
  final TextEditingController _orgCode = TextEditingController();
  final TextEditingController _username = TextEditingController();
  final TextEditingController _password = TextEditingController();

  bool _submitting = false;
  String? _error;
  bool _loadedOrgCode = false;

  @override
  void initState() {
    super.initState();
    _orgCode.addListener(_onChanged);
    _username.addListener(_onChanged);
    _password.addListener(_onChanged);
    unawaited(_loadLastOrgCode());
  }

  @override
  void dispose() {
    _orgCode.dispose();
    _username.dispose();
    _password.dispose();
    super.dispose();
  }

  Future<void> _loadLastOrgCode() async {
    final storage = ref.read(secureStorageProvider);
    final last = await storage.readLastOrgCode();
    if (!mounted) return;
    if (last != null && last.isNotEmpty && _orgCode.text.isEmpty) {
      _orgCode.text = last;
    }
    setState(() => _loadedOrgCode = true);
  }

  void _onChanged() => setState(() {});

  bool get _canSubmit {
    if (_submitting) return false;
    return _orgCode.text.trim().isNotEmpty &&
        _username.text.trim().isNotEmpty &&
        _password.text.isNotEmpty;
  }

  Future<void> _submit() async {
    if (!_canSubmit) return;
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      await ref.read(authProvider.notifier).login(
            orgCode: _orgCode.text.trim(),
            username: _username.text.trim(),
            password: _password.text,
          );
      // Router redirect handles navigation on success.
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() => _error = e.friendlyZh(context));
    } catch (_) {
      if (!mounted) return;
      setState(() => _error = AppLocalizations.of(context).errorGeneric);
    } finally {
      if (mounted) {
        setState(() => _submitting = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Scaffold(
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 32),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              const SizedBox(height: 24),
              _ArgusLogo(onSecretTapped: _onLogoTapped),
              const SizedBox(height: 8),
              Text(
                l10n.loginTitle,
                style: Theme.of(context).textTheme.headlineSmall,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 32),
              TextField(
                key: const Key('login.org_code'),
                controller: _orgCode,
                enabled: _loadedOrgCode && !_submitting,
                decoration: InputDecoration(
                  labelText: l10n.loginOrgCodeLabel,
                  border: const OutlineInputBorder(),
                ),
                textInputAction: TextInputAction.next,
                autocorrect: false,
                enableSuggestions: false,
              ),
              const SizedBox(height: 16),
              TextField(
                key: const Key('login.username'),
                controller: _username,
                enabled: !_submitting,
                decoration: InputDecoration(
                  labelText: l10n.loginUsernameLabel,
                  border: const OutlineInputBorder(),
                ),
                textInputAction: TextInputAction.next,
                autocorrect: false,
                enableSuggestions: false,
              ),
              const SizedBox(height: 16),
              TextField(
                key: const Key('login.password'),
                controller: _password,
                enabled: !_submitting,
                decoration: InputDecoration(
                  labelText: l10n.loginPasswordLabel,
                  border: const OutlineInputBorder(),
                ),
                obscureText: true,
                textInputAction: TextInputAction.done,
                onSubmitted: (_) => _submit(),
              ),
              if (_error != null) ...<Widget>[
                const SizedBox(height: 12),
                Text(
                  _error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ],
              const SizedBox(height: 24),
              FilledButton(
                key: const Key('login.submit'),
                onPressed: _canSubmit ? _submit : null,
                child: _submitting
                    ? const SizedBox(
                        height: 20,
                        width: 20,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Text(l10n.loginSubmit),
              ),
              if (kDebugMode) ...<Widget>[
                const SizedBox(height: 24),
                _DebugApiUrl(prefix: l10n.devMenuApiPrefix),
              ],
            ],
          ),
        ),
      ),
    );
  }

  void _onLogoTapped() {
    if (!kDebugMode) return;
    context.go(AppRoutes.devServerConfig);
  }
}

/// "Argus" logo + title. Tapped 5 times within 3 seconds opens the dev menu
/// in debug builds; release builds inert (the page itself is gated too).
class _ArgusLogo extends StatefulWidget {
  const _ArgusLogo({required this.onSecretTapped});

  final VoidCallback onSecretTapped;

  @override
  State<_ArgusLogo> createState() => _ArgusLogoState();
}

class _ArgusLogoState extends State<_ArgusLogo> {
  static const int _tapsRequired = 5;
  static const Duration _window = Duration(seconds: 3);

  final List<DateTime> _taps = <DateTime>[];

  void _handleTap() {
    if (!kDebugMode) return;
    final now = DateTime.now();
    _taps
      ..add(now)
      ..removeWhere((t) => now.difference(t) > _window);
    if (_taps.length >= _tapsRequired) {
      _taps.clear();
      widget.onSecretTapped();
    }
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: _handleTap,
      child: Center(
        child: Column(
          children: <Widget>[
            Icon(
              Icons.shield_outlined,
              size: 64,
              color: Theme.of(context).colorScheme.primary,
            ),
            const SizedBox(height: 8),
            Text(
              'Argus',
              style: Theme.of(context).textTheme.titleLarge,
            ),
          ],
        ),
      ),
    );
  }
}

class _DebugApiUrl extends ConsumerWidget {
  const _DebugApiUrl({required this.prefix});

  final String prefix;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final urlAsync = ref.watch(effectiveBaseUrlProvider);
    return Center(
      child: urlAsync.maybeWhen(
        data: (u) => Text(
          '$prefix: $u',
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.outline,
              ),
        ),
        orElse: () => const SizedBox.shrink(),
      ),
    );
  }
}
