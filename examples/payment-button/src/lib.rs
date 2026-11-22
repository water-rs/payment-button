//! Payment button example demonstrating `waterui_payment_button::PaymentButton`
//! on macOS, iOS, and Android.

use waterui::{
    Environment,
    app::{App, Windows},
    prelude::*,
    preview,
    reactive::binding,
};
use waterui_payment_button::{PaymentButton, PaymentButtonStyle, PaymentButtonType};

/// Root demo view displaying the native payment button and live type toggle.
#[preview]
pub fn demo() -> impl View {
    let button_type = binding(PaymentButtonType::Buy);
    let presses: Binding<u32> = binding(0_u32);
    let toggle_type = button_type.clone();
    let count_for_press = presses.clone();

    vstack((
        text!("Presses: {presses}").title(),
        PaymentButton::new(button_type)
            .style(PaymentButtonStyle::Automatic)
            .on_press(move || {
                count_for_press.with_mut(|count| *count += 1);
            }),
        button("Switch to Donate").action(move || {
            toggle_type.set(PaymentButtonType::Donate);
        }),
    ))
    .spacing(16.0)
    .padding()
}

/// Application entry point.
#[must_use]
pub fn app(env: Environment) -> App<impl Windows> {
    App::new(demo, env)
}
