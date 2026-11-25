# Contributing to Kirei

Thank you for your interest in Kirei. This guide covers setting up your environment, what we look for in a pull request and how to report issues. Issues and pull requests are welcome in English, Portuguese or Spanish.

## Setting up

1. Install [Rust](https://rustup.rs) and the Xcode Command Line Tools (`xcode-select --install`). A full Xcode install is not required.
2. Clone the repository and build:

   ```bash
   git clone https://github.com/shiddo-tech/kirei.git
   cd kirei
   scripts/bundle-macos.sh --install
   ```

3. If you rebuild often, create the local **Kirei Local Signing** certificate. Without it, macOS asks for the Screen Recording permission again after every build. See [Local code signing and permissions](README.md#local-code-signing-and-permissions) in the README for step-by-step instructions.

You do not need an Apple Developer account to contribute.

## Before opening a pull request

- For significant changes (new features, refactors, behavior changes), open an issue first so we can agree on the approach before anyone invests time.
- Keep each pull request focused on a single topic.
- Make sure `cargo build` succeeds and `cargo test` passes.
- Test with the `.app` bundle produced by `scripts/bundle-macos.sh`. Under `cargo run`, the Screen Recording permission is granted to your terminal instead.
- If you touch the renderer or scrolling capture, use the [test modes](README.md#test-modes) described in the README.
- For visual changes, attach before and after screenshots to the pull request.

## Code style

- Follow the conventions of the surrounding code. Run `cargo fmt` before committing, and discuss any new dependency in an issue first.
- Only write comments when the code alone cannot explain the rule, such as a macOS bug workaround or an external constraint.
- All user-facing text goes through `l("English text")`. The English text is the key.

## Translations

Kirei ships in English, Portuguese (Brazil) and Spanish. When you add or change a string, update all three files in `Localization/<language>.lproj/Localizable.strings`. If you can't translate a string, leave the English key in place and mention it in the pull request.

To add a new language, follow the steps under [Localization](README.md#localization) in the README.

## Privacy

Kirei runs entirely on the Mac, with no account, no server and no data collection. We do not accept pull requests that add telemetry, analytics or network calls the user has not explicitly asked for.

## Reporting a bug

Use the bug template under [Issues](https://github.com/shiddo-tech/kirei/issues/new/choose) and include:

- your Kirei version (shown in **Settings**) and macOS version;
- steps to reproduce the problem;
- the expected result and what actually happened.

If the app crashed or froze, attach the reports from `~/Library/Logs/Kirei`. Before attaching screenshots, make sure they don't reveal any personal data.

## License

By contributing, you agree that your contributions will be licensed under the project's [MIT License](LICENSE).
