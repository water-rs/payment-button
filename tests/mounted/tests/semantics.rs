//! Mounted `waterui-testing` semantic tests for `waterui-payment-button`.
//!
//! Only compiled on platforms where `PaymentButton` exists (macOS, iOS, Android).

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
mod supported {
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use waterui::ViewExt as _;
    use waterui::reactive::binding;
    use waterui_payment_button::{PaymentButton, PaymentButtonType};
    use waterui_testing::{Role, ui};

    #[test]
    fn payment_button_semantics_and_reactive_type_are_reachable() {
        let button_type = binding(PaymentButtonType::Buy);
        let type_for_view = button_type.clone();

        let mut app = ui().mount(move || PaymentButton::new(type_for_view.clone()));

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let (initial, updated) = ("Buy with Apple Pay", "Subscribe with Apple Pay");
        #[cfg(target_os = "android")]
        let (initial, updated) = ("Buy with Google Pay", "Subscribe with Google Pay");

        app.query()
            .role(Role::BUTTON)
            .label(initial)
            .assert_exists();

        button_type.set(PaymentButtonType::Subscribe);
        app.query()
            .role(Role::BUTTON)
            .label(updated)
            .assert_exists();
    }

    #[test]
    fn payment_button_outer_a11y_label_overrides_default() {
        let mut app = ui().mount(|| {
            PaymentButton::new(PaymentButtonType::Donate).a11y_label("Support the Project")
        });

        app.query()
            .role(Role::BUTTON)
            .label("Support the Project")
            .assert_exists();
    }

    #[test]
    fn payment_button_tap_invokes_on_press() {
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_for_press = Arc::clone(&hits);

        let mut app = ui().mount(move || {
            let hits_inner = Arc::clone(&hits_for_press);
            PaymentButton::new(PaymentButtonType::Pay).on_press(move || {
                hits_inner.fetch_add(1, Ordering::SeqCst);
            })
        });

        app.query().role(Role::BUTTON).tap();
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
