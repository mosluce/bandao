import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../../core/api/api_client.dart';
import '../../../core/env/env.dart';
import '../../../core/storage/api_base_url.dart';
import '../../../core/storage/dev_overrides.dart';
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
  bool _initialized = false;
  String? _error;

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
    if (!mounted) return;
    _input.text = (saved == null || saved.isEmpty)
        ? Env.compileTimeDefault()
        : saved;
    setState(() => _initialized = true);
  }

  @override
  void dispose() {
    _input.dispose();
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

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final effectiveAsync = ref.watch(effectiveBaseUrlProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.devMenuTitle)),
      body: SafeArea(
        child: Padding(
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
                  ],
                ),
        ),
      ),
    );
  }
}
