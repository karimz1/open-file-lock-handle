# Desktop translations

These files contain the languages available in the OFLH desktop app. Select a language in Settings, or use the system language setting.

## Languages and credits

| Language                      | Code | File           | Translation credit                    |
| ----------------------------- | ---- | -------------- | ------------------------------------- |
| English                       | `en` | [en.ts](en.ts) | [karimz1](https://github.com/karimz1) |
| German (Deutsch)              | `de` | [de.ts](de.ts) | Automatic translation                 |
| Simplified Chinese (简体中文) | `zh` | [zh.ts](zh.ts) | Automatic translation                 |

English is the source language. German and Simplified Chinese were translated using automatic translators, so some words or phrases may be imperfect. Corrections from speakers of these languages are welcome.

## Fix a translation

If something looks wrong, sounds unnatural, or is unclear, edit the corresponding language file and open a pull request. You can also [open an issue](https://github.com/karimz1/open-file-lock-handle/issues/new) with the language, current wording, suggested correction, and where it appears.

Translate the message values while keeping the keys and nested structure unchanged. Preserve placeholders such as `{{version}}`, including their names and double braces. Keep the meaning of safety warnings and the distinction between file usage and proven locks.

## Add a language

1. Copy [en.ts](en.ts) to a new file named for the language code, such as `fr.ts`. Rename the catalog variable and default export to match, and remove the English-only `MessageKey` type export. Follow [de.ts](de.ts) to type the catalog as `TranslationSchema<typeof en>` so TypeScript checks its structure.
2. Translate every message value. Keep all keys, groups, and interpolation placeholders identical to English; the app requires complete catalogs.
3. Add a language-name key in the `language` group of **every** catalog, including English, and translate its label in each language.
4. Import the new catalog in [../i18n.ts](../i18n.ts) and add it to `LANGUAGES` with its code, catalog, and language-name key. This also adds it to the Settings language selector. The current resolver uses the primary language subtag (for example, `fr-FR` becomes `fr`); regional or script-specific variants need resolver changes and tests.
5. Update [../i18n.test.ts](../i18n.test.ts): import the new catalog, include it in the completeness and placeholder checks, and test language resolution and representative translations.
6. Add the language and translation credits to the table above, and update the desktop language badge in the [main README](../../../../../README.md).
7. From `crates/oflh-desktop/ui`, run:

   ```sh
   npm ci
   npm run format:check
   npm test
   npm run build
   ```

   Launch the desktop app using the [development instructions](../../../../../docs/development.md#desktop-development) and review your language in Settings. Check long labels, dialogs, and safety warnings for clarity and fit. From the repository root, also run `cargo xtask check` before submitting the pull request.

In your pull request, mention which language you added or corrected, whether automatic translation was used, and how you would like to be credited. Contributions that improve existing translations are just as welcome as new languages.
