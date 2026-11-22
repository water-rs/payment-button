//! Official native payment button plugin for `WaterUI`.
//!
//! Renders `PKPaymentButton` via [`waterui_apple::AppleView`] on macOS and iOS,
//! and Google Pay's `PayButton` via `hydrolysis::AndroidView` on Android.
//! Intentionally omitted on unsupported platforms so applications using it
//! there fail to compile at the plugin's use.

#![cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]

#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;

use nami::signal::IntoComputed;
use waterui::handler::{Handler, SharedAction, shared_action};
use waterui::reactive::{Computed, SignalExt as _};
use waterui::{Environment, Str, View};

/// The semantic action label displayed on the native payment button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PaymentButtonType {
    /// Payment service mark with no leading action verb.
    Plain,
    /// "Buy with ..."
    #[default]
    Buy,
    /// "Pay with ..."
    Pay,
    /// "Checkout with ..."
    Checkout,
    /// "Book with ..."
    Book,
    /// "Subscribe with ..."
    Subscribe,
    /// "Donate with ..."
    Donate,
    /// "Order with ..."
    Order,
}

impl PaymentButtonType {
    #[cfg(target_os = "android")]
    pub(crate) const fn android_ordinal(self) -> i32 {
        match self {
            Self::Plain => 0,
            Self::Buy => 1,
            Self::Pay => 2,
            Self::Checkout => 3,
            Self::Book => 4,
            Self::Subscribe => 5,
            Self::Donate => 6,
            Self::Order => 7,
        }
    }

    pub(crate) const fn accessibility_label_text(self) -> &'static str {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            match self {
                Self::Plain => "Apple Pay",
                Self::Buy => "Buy with Apple Pay",
                Self::Pay => "Pay with Apple Pay",
                Self::Checkout => "Check out with Apple Pay",
                Self::Book => "Book with Apple Pay",
                Self::Subscribe => "Subscribe with Apple Pay",
                Self::Donate => "Donate with Apple Pay",
                Self::Order => "Order with Apple Pay",
            }
        }
        #[cfg(target_os = "android")]
        {
            match self {
                Self::Plain => "Google Pay",
                Self::Buy => "Buy with Google Pay",
                Self::Pay => "Pay with Google Pay",
                Self::Checkout => "Checkout with Google Pay",
                Self::Book => "Book with Google Pay",
                Self::Subscribe => "Subscribe with Google Pay",
                Self::Donate => "Donate with Google Pay",
                Self::Order => "Order with Google Pay",
            }
        }
    }
}

/// Visual appearance of the native payment button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PaymentButtonStyle {
    /// Adapts automatically to the environment's color scheme.
    #[default]
    Automatic,
    /// Light background with dark branding.
    Light,
    /// Light background with a dark hairline border (white-outline on Apple).
    LightOutline,
    /// Dark background with light branding.
    Dark,
}

nami::impl_constant!(PaymentButtonType, PaymentButtonStyle);

/// A native system payment button (`PKPaymentButton` on Apple, `PayButton` on Android).
#[derive(Debug, Clone)]
#[must_use]
pub struct PaymentButton {
    button_type: Computed<PaymentButtonType>,
    style: Computed<PaymentButtonStyle>,
    corner_radius: Option<f32>,
    #[cfg(target_os = "android")]
    allowed_payment_methods: Str,
    on_press: SharedAction<()>,
}

#[cfg(target_os = "android")]
const DEFAULT_ANDROID_ALLOWED_PAYMENT_METHODS: &str = r#"[{"type":"CARD","parameters":{"allowedAuthMethods":["PAN_ONLY","CRYPTOGRAM_3DS"],"allowedCardNetworks":["AMEX","DISCOVER","JCB","MASTERCARD","VISA"]}}]"#;

impl PaymentButton {
    /// Creates a payment button with a static or reactive [`PaymentButtonType`].
    pub fn new(button_type: impl IntoComputed<PaymentButtonType>) -> Self {
        Self {
            button_type: button_type.into_computed(),
            style: Computed::constant(PaymentButtonStyle::Automatic),
            corner_radius: None,
            #[cfg(target_os = "android")]
            allowed_payment_methods: Str::from_static(DEFAULT_ANDROID_ALLOWED_PAYMENT_METHODS),
            on_press: shared_action(|| {}),
        }
    }

    /// Sets the button's visual style (defaults to [`PaymentButtonStyle::Automatic`]).
    pub fn style(mut self, style: impl IntoComputed<PaymentButtonStyle>) -> Self {
        self.style = style.into_computed();
        self
    }

    /// Sets the corner radius in logical points (Apple) or dp (Android).
    pub const fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = Some(radius);
        self
    }

    /// Sets the Google Pay `allowedPaymentMethods` JSON array used on Android.
    ///
    /// Ignored on Apple platforms where `PKPaymentButton` configures appearance
    /// without a payment request.
    #[cfg_attr(not(target_os = "android"), allow(unused_mut))]
    pub fn allowed_payment_methods(mut self, json: impl Into<Str>) -> Self {
        #[cfg(target_os = "android")]
        {
            self.allowed_payment_methods = json.into();
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = json.into();
        }
        self
    }

    /// Installs the action invoked when the user activates the payment button.
    pub fn on_press<Args>(mut self, handler: impl Handler<Args, ()> + 'static) -> Self {
        self.on_press = shared_action(handler);
        self
    }

    pub(crate) fn accessibility_label_signal(&self) -> Computed<Str> {
        self.button_type
            .clone()
            .map(|kind| Str::from_static(kind.accessibility_label_text()))
            .into_computed()
    }
}

impl View for PaymentButton {
    fn body(self, env: &Environment) -> impl View {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            apple::payment_view(self, env)
        }
        #[cfg(target_os = "android")]
        {
            android::payment_view(self, env)
        }
    }
}
