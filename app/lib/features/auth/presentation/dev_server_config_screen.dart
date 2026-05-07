import 'package:firebase_crashlytics/firebase_crashlytics.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../../core/api/api_client.dart';
import '../../../core/env/env.dart';
import '../../../core/storage/api_base_url.dart';
import '../../../core/storage/dev_overrides.dart';
import '../../../core/storage/privacy_url.dart';
import '../../../core/storage/secure_storage.dart';
import '../../../l10n/app_localizations.dart';

/// Debug-only menu for swapping the API base URL at runtime. Reachable from
/// the login screen by tapping the logo 5 times within 3 seconds.
class DevServerConfigScreen extends ConsumerStatefulWidget {
  const DevServerConfigScreen({super.key});

  @override
  ConsumerState<DevServerConfigScreen> createState() =>
      _DevServerConfigScreenState();
}

class _DevServerConfigScreenState
    extends ConsumerState<DevServerConfigScreen> {
  final TextEditingController _input = TextEditingController();
  final TextEditingController _privacyInput = TextEditingController();
  bool _initialized = false;
  String? _error;
  String? _privacyError;

  @override
  void initState() {
    super.initState();
    _seed();
  }

  Future<void> _seed() async {
    if (kReleaseMode) {
      setState(() => _initialized = true);
      return;
    }
    final overrides = ref.read(devOverridesProvider);
    final saved = await overrides.read();
    final storage = ref.read(secureStorageProvider);
    final savedPrivacy = await storage.readPrivacyUrlOverride();
    if (!mounted) return;
    _input.text = (saved == null || saved.isEmpty)
        ? Env.compileTimeDefault()
        : saved;
    _privacyInput.text = (savedPrivacy == null || savedPrivacy.isEmpty)
        ? Env.privacyUrlCompileTimeDefault()
        : savedPrivacy;
    setState(() => _initialized = true);
  }

  @override
  void dispose() {
    _input.dispose();
    _privacyInput.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final url = _input.text.trim();
    final parsed = Uri.tryParse(url);
    if (parsed == null || !parsed.hasScheme || !parsed.hasAuthority) {
      setState(() => _error = AppLocalizations.of(context).errorGeneric);
      return;
    }
    setState(() => _error = null);
    final overrides = ref.read(devOverridesProvider);
    await overrides.write(url);
    // Force the dio client + url resolver to rebuild on next request.
    ref.invalidate(effectiveBaseUrlProvider);
    ref.invalidate(apiClientProvider);
    if (!mounted) return;
    if (context.canPop()) {
      context.pop();
    } else {
      context.go('/login');
    }
  }

  Future<void> _clear() async {
    final overrides = ref.read(devOverridesProvider);
    await overrides.clear();
    ref.invalidate(effectiveBaseUrlProvider);
    ref.invalidate(apiClientProvider);
    if (!mounted) return;
    if (context.canPop()) {
      context.pop();
    } else {
      context.go('/login');
    }
  }

  Future<void> _savePrivacy() async {
    final url = _privacyInput.text.trim();
    final parsed = Uri.tryParse(url);
    if (parsed == null || !parsed.hasScheme || !parsed.hasAuthority) {
      setState(() => _privacyError = AppLocalizations.of(context).errorGeneric);
      return;
    }
    setState(() => _privacyError = null);
    final storage = ref.read(secureStorageProvider);
    await storage.writePrivacyUrlOverride(url);
    ref.invalidate(effectivePrivacyUrlProvider);
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(AppLocalizations.of(context).devMenuSaved)),
    );
  }

  Future<void> _clearPrivacy() async {
    final storage = ref.read(secureStorageProvider);
    await storage.clearPrivacyUrlOverride();
    ref.invalidate(effectivePrivacyUrlProvider);
    _privacyInput.text = Env.privacyUrlCompileTimeDefault();
    if (!mounted) return;
    setState(() => _privacyError = null);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final effectiveAsync = ref.watch(effectiveBaseUrlProvider);
    final effectivePrivacyAsync = ref.watch(effectivePrivacyUrlProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.devMenuTitle)),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: !_initialized
              ? const Center(child: CircularProgressIndicator())
              : Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    Text(
                      l10n.devMenuCurrentLabel,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 4),
                    effectiveAsync.maybeWhen(
                      data: (u) => SelectableText(u),
                      orElse: () => const SizedBox.shrink(),
                    ),
                    const SizedBox(height: 24),
                    TextField(
                      controller: _input,
                      decoration: InputDecoration(
                        labelText: l10n.devMenuInputLabel,
                        border: const OutlineInputBorder(),
                      ),
                      keyboardType: TextInputType.url,
                      autocorrect: false,
                    ),
                    if (_error != null) ...<Widget>[
                      const SizedBox(height: 12),
                      Text(
                        _error!,
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ),
                    ],
                    const SizedBox(height: 16),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: <Widget>[
                        TextButton(
                          onPressed: _clear,
                          child: Text(l10n.devMenuClear),
                        ),
                        const SizedBox(width: 8),
                        FilledButton(
                          onPressed: _save,
                          child: Text(l10n.devMenuSave),
                        ),
                      ],
                    ),
                    const Divider(height: 48),
                    Text(
                      l10n.devMenuPrivacyCurrentLabel,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    const SizedBox(height: 4),
                    effectivePrivacyAsync.maybeWhen(
                      data: (u) => SelectableText(u),
                      orElse: () => const SizedBox.shrink(),
                    ),
                    const SizedBox(height: 16),
                    TextField(
                      controller: _privacyInput,
                      decoration: InputDecoration(
                        labelText: l10n.devMenuPrivacyInputLabel,
                        border: const OutlineInputBorder(),
                      ),
                      keyboardType: TextInputType.url,
                      autocorrect: false,
                    ),
                    if (_privacyError != null) ...<Widget>[
                      const SizedBox(height: 12),
                      Text(
                        _privacyError!,
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ),
                    ],
                    const SizedBox(height: 16),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: <Widget>[
                        TextButton(
                          onPressed: _clearPrivacy,
                          child: Text(l10n.devMenuClear),
                        ),
                        const SizedBox(width: 8),
                        FilledButton(
                          onPressed: _savePrivacy,
                          child: Text(l10n.devMenuSave),
                        ),
                      ],
                    ),
                    if (kDebugMode) ...<Widget>[
                      const Divider(height: 48),
                      Text(
                        'Crashlytics 自我測試',
                        style: Theme.of(context).textTheme.titleSmall,
                      ),
                      const SizedBox(height: 4),
                      const Text(
                        '此按鈕僅在 debug build 出現；release build 不存在。按下後會強制觸發一個原生 crash，幾分鐘內應在 Firebase Console 看到對應紀錄。',
                        style: TextStyle(fontSize: 12),
                      ),
                      const SizedBox(height: 12),
                      ElevatedButton.icon(
                        onPressed: () => FirebaseCrashlytics.instance.crash(),
                        icon: const Icon(Icons.bug_report),
                        label: const Text('強制觸發 Crash（測試 Crashlytics）'),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: Colors.red,
                          foregroundColor: Colors.white,
                        ),
                      ),
                    ],
                  ],
                ),
        ),
      ),
    );
  }
}
