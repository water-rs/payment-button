# waterui-payment-button

Official native payment button plugin for WaterUI, built on `AppleView` (`PKPaymentButton` on macOS and iOS) and `AndroidView` (Google Pay `PayButton` on Android).

## Platform Support

- **macOS / iOS**: `PKPaymentButton` via `waterui_apple::AppleContent` + `AppleView`, requesting `apple_pay_merchant_ids` (`com.apple.developer.in-app-payments`) through `[[package.metadata.waterui.app-value]]`.
- **Android**: Google Pay `PayButton` via `hydrolysis::AndroidContent` + `AndroidView`, declaring `com.google.android.gms:play-services-wallet`, `com.google.android.gms.wallet.api.enabled`, and exact R8 keep rules.
- **Other targets (Linux, Windows, Web)**: `PaymentButton` and its configuration types are intentionally absent so applications using it fail to compile at the plugin's use rather than substituting a non-compliant stand-in.
