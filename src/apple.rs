//! Apple (`macOS` and `iOS`) implementation of [`crate::PaymentButton`] on
//! top of `PassKit::PKPaymentButton` and [`waterui_apple::AppleView`].

use core::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2_core_foundation::{CGFloat, CGRect, CGSize};
#[cfg(target_os = "macos")]
use objc2_pass_kit::PKPaymentButton;
use objc2_pass_kit::{PKPaymentButtonStyle, PKPaymentButtonType};
use waterui::accessibility::AccessibilityRole;
use waterui::handler::SharedAction;
use waterui::reactive::{Computed, SignalExt as _};
use waterui::theme::{ColorScheme, current_color_scheme};
use waterui::{Environment, Str, View};
use waterui_apple::objc2::rc::Retained;
use waterui_apple::objc2::runtime::{AnyObject, NSObject};
#[cfg(target_os = "ios")]
use waterui_apple::objc2::{ClassType as _, extern_class};
use waterui_apple::objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
#[cfg(target_os = "macos")]
use waterui_apple::objc2_app_kit::{NSAutoresizingMaskOptions, NSControl, NSView};
#[cfg(target_os = "ios")]
use waterui_apple::objc2_ui_kit::{
    UIButton, UIControl, UIControlEvents, UIResponder, UIView, UIViewAutoresizing,
};
use waterui_apple::{AppleContent, AppleMount, AppleView, PlatformView};
use waterui_core::layout::{ProposalSize, Size, StretchAxis, ViewDimensions};

use crate::{PaymentButton, PaymentButtonStyle, PaymentButtonType};

#[cfg(target_os = "ios")]
#[link(name = "PassKit", kind = "framework")]
unsafe extern "C" {}

#[cfg(target_os = "ios")]
extern_class!(
    /// PassKit's `PKPaymentButton` on iOS, inheriting from `UIButton`.
    #[unsafe(super(UIButton, UIControl, UIView, UIResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    struct PKPaymentButton;
);

#[cfg(target_os = "ios")]
impl PKPaymentButton {
    fn button_with_type_style(
        button_type: PKPaymentButtonType,
        button_style: PKPaymentButtonStyle,
        _mtm: MainThreadMarker,
    ) -> Retained<Self> {
        // SAFETY: `PKPaymentButton` exposes `+buttonWithType:style:` returning a
        // retained `+1` autoreleased/retained instance on the main thread.
        unsafe { msg_send![Self::class(), buttonWithType: button_type, style: button_style] }
    }

    fn set_corner_radius(&self, corner_radius: CGFloat) {
        // SAFETY: `PKPaymentButton` exposes `-setCornerRadius:` taking `CGFloat`.
        unsafe { msg_send![self, setCornerRadius: corner_radius] }
    }
}

#[derive(Debug)]
struct ActionTargetIvars {
    action: SharedAction<()>,
    env: Environment,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ActionTargetIvars]
    #[derive(Debug)]
    struct ActionTarget;

    impl ActionTarget {
        #[unsafe(method(onPress:))]
        fn on_press(&self, _sender: &AnyObject) {
            self.ivars().action.run(&self.ivars().env);
        }
    }
);

impl ActionTarget {
    fn new(mtm: MainThreadMarker, action: SharedAction<()>, env: Environment) -> Retained<Self> {
        let this = mtm
            .alloc::<Self>()
            .set_ivars(ActionTargetIvars { action, env });
        // SAFETY: `NSObject`'s designated initializer `-init` has no preconditions
        // after `set_ivars`.
        unsafe { msg_send![super(this), init] }
    }
}

struct TargetGuard {
    target: Retained<ActionTarget>,
    active_button: Rc<RefCell<Option<Retained<PKPaymentButton>>>>,
}

impl Drop for TargetGuard {
    fn drop(&mut self) {
        if let Some(button) = self.active_button.borrow().as_ref() {
            detach_target(button, &self.target);
        }
    }
}

struct ApplePaymentContent {
    button_type: Computed<PaymentButtonType>,
    style: Computed<PaymentButtonStyle>,
    corner_radius: Option<f32>,
    action: SharedAction<()>,
    env: Environment,
    accessibility_label: Computed<Str>,
    active_button: Rc<RefCell<Option<Retained<PKPaymentButton>>>>,
}

pub fn payment_view(button: PaymentButton, env: &Environment) -> impl View {
    let accessibility_label = button.accessibility_label_signal();
    AppleView::new(ApplePaymentContent {
        button_type: button.button_type,
        style: button.style,
        corner_radius: button.corner_radius,
        action: button.on_press,
        env: env.clone(),
        accessibility_label,
        active_button: Rc::new(RefCell::new(None)),
    })
}

const fn map_button_type(kind: PaymentButtonType) -> PKPaymentButtonType {
    match kind {
        // PassKit has no dedicated `Pay` constant; `Plain` is the canonical
        // Apple Pay mark without an alternate verb.
        PaymentButtonType::Plain | PaymentButtonType::Pay => PKPaymentButtonType::Plain,
        PaymentButtonType::Buy => PKPaymentButtonType::Buy,
        PaymentButtonType::Checkout => PKPaymentButtonType::Checkout,
        PaymentButtonType::Book => PKPaymentButtonType::Book,
        PaymentButtonType::Subscribe => PKPaymentButtonType::Subscribe,
        PaymentButtonType::Donate => PKPaymentButtonType::Donate,
        PaymentButtonType::Order => PKPaymentButtonType::Order,
    }
}

const fn map_button_style(
    style: PaymentButtonStyle,
    color_scheme: ColorScheme,
) -> PKPaymentButtonStyle {
    match style {
        PaymentButtonStyle::Automatic => match color_scheme {
            ColorScheme::Light => PKPaymentButtonStyle::Black,
            ColorScheme::Dark => PKPaymentButtonStyle::White,
        },
        PaymentButtonStyle::Light => PKPaymentButtonStyle::White,
        PaymentButtonStyle::LightOutline => PKPaymentButtonStyle::WhiteOutline,
        PaymentButtonStyle::Dark => PKPaymentButtonStyle::Black,
    }
}

fn create_pk_button(
    mtm: MainThreadMarker,
    kind: PaymentButtonType,
    style: PaymentButtonStyle,
    color_scheme: ColorScheme,
    corner_radius: Option<f32>,
    target: &ActionTarget,
) -> Retained<PKPaymentButton> {
    let pk_type = map_button_type(kind);
    let pk_style = map_button_style(style, color_scheme);

    #[cfg(target_os = "macos")]
    let button = {
        // SAFETY: `mtm` proves we are on the main thread; `pk_type` and
        // `pk_style` are valid `PKConstants` enum values.
        let button = unsafe { PKPaymentButton::buttonWithType_style(pk_type, pk_style, mtm) };
        if let Some(radius) = corner_radius {
            // SAFETY: `radius` is finite or coerced safely by PassKit.
            unsafe { button.setCornerRadius(CGFloat::from(radius)) };
        }
        let target_obj: &AnyObject = target;
        // SAFETY: `target` stays retained by `TargetGuard` via `mount.keep`, and
        // `detach_target` clears `target` before `TargetGuard` drops.
        unsafe {
            let control: &NSControl = &button;
            control.setTarget(Some(target_obj));
            control.setAction(Some(sel!(onPress:)));
        }
        button.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        button
    };

    #[cfg(target_os = "ios")]
    let button = {
        let button = PKPaymentButton::button_with_type_style(pk_type, pk_style, mtm);
        if let Some(radius) = corner_radius {
            button.set_corner_radius(CGFloat::from(radius));
        }
        let target_obj: &AnyObject = target;
        // SAFETY: `target` stays retained by `TargetGuard` via `mount.keep`, and
        // `detach_target` removes the target before `TargetGuard` drops.
        unsafe {
            let control: &UIControl = &button;
            control.addTarget_action_forControlEvents(
                Some(target_obj),
                sel!(onPress:),
                UIControlEvents::TouchUpInside,
            );
        }
        button.setAutoresizingMask(
            UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight,
        );
        button
    };

    button
}

fn detach_target(button: &PKPaymentButton, target: &ActionTarget) {
    #[cfg(target_os = "macos")]
    {
        let _ = target;
        // SAFETY: Clearing target and action on an `NSControl` is always safe on
        // the main thread.
        unsafe {
            let control: &NSControl = button;
            control.setTarget(None);
            control.setAction(None);
        }
    }

    #[cfg(target_os = "ios")]
    {
        let target_obj: &AnyObject = target;
        // SAFETY: Removing a previously registered target-action pair on the
        // main thread is safe.
        unsafe {
            let control: &UIControl = button;
            control.removeTarget_action_forControlEvents(
                Some(target_obj),
                Some(sel!(onPress:)),
                UIControlEvents::TouchUpInside,
            );
        }
    }
}

fn fitting_size(button: &PKPaymentButton) -> CGSize {
    #[cfg(target_os = "macos")]
    {
        button.fittingSize()
    }
    #[cfg(target_os = "ios")]
    {
        button.systemLayoutSizeFittingSize(CGSize::ZERO)
    }
}

impl AppleContent for ApplePaymentContent {
    type View = PlatformView;

    fn make_view(&mut self, mount: &mut AppleMount<'_>) -> Retained<Self::View> {
        let mtm = mount.mtm();
        #[cfg(target_os = "macos")]
        let container: Retained<PlatformView> = {
            let view = NSView::initWithFrame(NSView::alloc(mtm), CGRect::ZERO);
            view.setAutoresizesSubviews(true);
            view
        };
        #[cfg(target_os = "ios")]
        let container: Retained<PlatformView> = {
            let view = UIView::initWithFrame(UIView::alloc(mtm), CGRect::ZERO);
            view.setAutoresizesSubviews(true);
            view
        };

        let target = ActionTarget::new(mtm, self.action.clone(), mount.env().clone());
        let active_button = Rc::clone(&self.active_button);
        mount.keep(TargetGuard {
            target: target.clone(),
            active_button: Rc::clone(&active_button),
        });

        let invalidator = mount.invalidator();
        let did_mount = Cell::new(false);
        let corner_radius = self.corner_radius;
        let container_ref = container.clone();
        let color_scheme_signal = current_color_scheme(mount.env());
        let combined = self
            .button_type
            .clone()
            .zip(&self.style)
            .zip(&color_scheme_signal);

        mount.bind(&combined, move |((kind, style), color_scheme)| {
            if let Some(previous) = active_button.borrow_mut().take() {
                detach_target(&previous, &target);
                previous.removeFromSuperview();
            }

            let child = create_pk_button(mtm, kind, style, color_scheme, corner_radius, &target);
            child.setFrame(container_ref.bounds());
            #[cfg(target_os = "macos")]
            {
                let child_view: &NSView = &child;
                container_ref.addSubview(child_view);
            }
            #[cfg(target_os = "ios")]
            {
                let child_view: &UIView = &child;
                container_ref.addSubview(child_view);
            }

            *active_button.borrow_mut() = Some(child);
            if did_mount.replace(true) {
                invalidator.invalidate_layout();
                invalidator.invalidate_display();
            }
        });

        container
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "WaterUI layout coordinates are f32 points; AppKit/UIKit fitting sizes are f64 points"
    )]
    fn measure(&self, _view: &Self::View, proposal: ProposalSize) -> ViewDimensions {
        let size = self
            .active_button
            .borrow()
            .as_deref()
            .map_or(CGSize::ZERO, fitting_size);
        let fit_width = (size.width as f32).max(0.0);
        let fit_height = (size.height as f32).max(0.0);
        let width = proposal
            .width
            .map_or(fit_width, |offered| offered.max(fit_width));
        ViewDimensions::new(Size::new(width, fit_height))
    }

    fn stretch_axis(&self) -> StretchAxis {
        StretchAxis::Horizontal
    }

    fn accessibility_role(&self) -> Option<AccessibilityRole> {
        Some(AccessibilityRole::Button)
    }

    fn accessibility_label(&self) -> Option<Computed<Str>> {
        Some(self.accessibility_label.clone())
    }

    fn accessibility_activate(&self) -> Option<Rc<dyn Fn()>> {
        let action = self.action.clone();
        let env = self.env.clone();
        Some(Rc::new(move || action.run(&env)))
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn style_follows_color_scheme() {
        assert_eq!(
            map_button_style(PaymentButtonStyle::Automatic, ColorScheme::Light),
            PKPaymentButtonStyle::Black
        );
        assert_eq!(
            map_button_style(PaymentButtonStyle::Automatic, ColorScheme::Dark),
            PKPaymentButtonStyle::White
        );
        assert_eq!(
            map_button_style(PaymentButtonStyle::LightOutline, ColorScheme::Dark),
            PKPaymentButtonStyle::WhiteOutline
        );
    }
}
