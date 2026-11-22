# AGENTS.md

Rules for working in `waterui-payment-button`.

- This crate is a WaterUI plugin built solely on public contracts (`waterui`, `waterui_apple::AppleContent`/`AppleView`, `hydrolysis::AndroidContent`/`AndroidView`, and `[package.metadata.waterui.*]` component declarations). Never add custom backend traits or private FFI hooks into WaterUI.
- `PaymentButton` exists only on `target_os = "macos"`, `target_os = "ios"`, and `target_os = "android"`. Never provide a fallback implementation or stub for unsupported targets.
- Keep `Cargo.lock` committed once `Cargo.toml` lands (and keep `.gitignore` limited to `/target`). Every dependency uses `default-features = false` with only the needed features.
- Do not launch GUI windows on the developer MacBook Pro; verify macOS/iOS via `cargo clippy`, headless unit tests, and `waterui-testing`'s semantic runtime, Android via `cargo clippy --target aarch64-linux-android`, and unsupported targets via Linux compilation + example compile-fail checks.
- All commits must be GPG-signed (`git commit -S`) and landed via pull request to `main`.
