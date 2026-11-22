//! Main-thread headless verification of `PaymentButton` through `AppleBackend` on macOS.

use std::env;

fn should_list_only() -> bool {
    env::args().any(|arg| arg == "--list")
}

#[cfg(target_os = "macos")]
fn main() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use objc2_pass_kit::PKPaymentButton;
    use waterui::reactive::Binding;
    use waterui::theme::{ColorScheme, install_color_scheme};
    use waterui::{Environment, SignalExt as _, binding};
    use waterui_apple::MainThreadMarker;
    use waterui_apple::backend::AppleBackend;
    use waterui_apple::objc2::rc::Retained;
    use waterui_apple::objc2_app_kit::NSControl;
    use waterui_backend_core::Realize;
    use waterui_core::layout::{ProposalSize, StretchAxis};
    use waterui_payment_button::{PaymentButton, PaymentButtonStyle, PaymentButtonType};

    if should_list_only() {
        println!("apple_button: test");
        return;
    }

    let mtm = MainThreadMarker::new().expect("harness=false main runs on the macOS main thread");
    let mut backend = AppleBackend::new(mtm);
    let mut env = Environment::new();
    let scheme: Binding<ColorScheme> = binding(ColorScheme::Light);
    install_color_scheme(&mut env, scheme.computed());

    let kind: Binding<PaymentButtonType> = binding(PaymentButtonType::Buy);
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_clone = Arc::clone(&hits);

    let view = PaymentButton::new(kind.clone())
        .style(PaymentButtonStyle::Automatic)
        .corner_radius(6.0)
        .on_press(move || {
            hits_clone.fetch_add(1, Ordering::SeqCst);
        });

    let leaf = Realize::child(&mut backend, view, &env, ());
    assert_eq!(leaf.layout().stretch_axis(), StretchAxis::Horizontal);

    let natural = leaf.layout().measure(ProposalSize::UNSPECIFIED);
    assert!(
        natural.size.width > 0.0 && natural.size.height > 0.0,
        "expected positive intrinsic size, got {natural:?}"
    );

    let stretched = leaf.layout().measure(ProposalSize::new(Some(320.0), None));
    assert!(
        (stretched.size.width - 320.0).abs() < f32::EPSILON,
        "expected horizontal stretch to 320.0, got {stretched:?}"
    );
    assert!(
        (stretched.size.height - natural.size.height).abs() < f32::EPSILON,
        "expected intrinsic height preservation, got {stretched:?}"
    );

    let host_subviews = leaf.view().subviews();
    assert_eq!(host_subviews.len(), 1);
    let container = host_subviews.objectAtIndex(0);

    let subviews = container.subviews();
    assert_eq!(subviews.len(), 1);
    let first = subviews.objectAtIndex(0);
    let pk_button = first
        .downcast::<PKPaymentButton>()
        .expect("child subview must be a PKPaymentButton");
    let control: &NSControl = &pk_button;
    // SAFETY: `pk_button` is an initialized `NSControl` owned on the main thread.
    unsafe { control.performClick(None) };
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let first_ptr = Retained::as_ptr(&pk_button);
    kind.set(PaymentButtonType::Donate);
    let updated_subviews = container.subviews();
    assert_eq!(updated_subviews.len(), 1);
    let second = updated_subviews
        .objectAtIndex(0)
        .downcast::<PKPaymentButton>()
        .expect("updated child subview must be a PKPaymentButton");
    let second_ptr = Retained::as_ptr(&second);
    assert_ne!(
        second_ptr, first_ptr,
        "changing PaymentButtonType must swap the underlying PKPaymentButton"
    );
    let second_control: &NSControl = &second;
    // SAFETY: `second` is the newly mounted `PKPaymentButton` on the main thread.
    unsafe { second_control.performClick(None) };
    assert_eq!(hits.load(Ordering::SeqCst), 2);

    scheme.set(ColorScheme::Dark);
    let dark_subviews = container.subviews();
    assert_eq!(dark_subviews.len(), 1);
    let third = dark_subviews
        .objectAtIndex(0)
        .downcast::<PKPaymentButton>()
        .expect("dark-mode child subview must be a PKPaymentButton");
    assert_ne!(
        Retained::as_ptr(&third),
        second_ptr,
        "changing ColorScheme with Automatic style must swap the underlying PKPaymentButton"
    );
    let third_control: &NSControl = &third;
    // SAFETY: `third` is the dark-mode `PKPaymentButton` on the main thread.
    unsafe { third_control.performClick(None) };
    assert_eq!(hits.load(Ordering::SeqCst), 3);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    if should_list_only() {
        println!("apple_button: test");
    }
}
