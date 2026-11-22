//! Android implementation of [`crate::PaymentButton`] on top of Google Pay's
//! `PayButton` (`com.google.android.gms.wallet.button.PayButton`) and
//! [`hydrolysis::AndroidView`].

use core::cell::Cell;
use core::ffi::c_void;
use std::rc::Rc;

use hydrolysis::jni::errors::{Error as JniError, Result as JniResult, ThrowRuntimeExAndDefault};
use hydrolysis::jni::objects::{Global, JClass, JObject, JValue};
use hydrolysis::jni::sys::jlong;
use hydrolysis::jni::{Env, EnvUnowned, JavaVM, NativeMethod, jni_sig, jni_str};
use hydrolysis::{AndroidContent, AndroidMount, AndroidView};
use waterui::accessibility::AccessibilityRole;
use waterui::handler::SharedAction;
use waterui::reactive::{Computed, Signal as _, SignalExt as _};
use waterui::theme::{ColorScheme, current_color_scheme};
use waterui::{Environment, Str, View};
use waterui_core::layout::StretchAxis;

use crate::{PaymentButton, PaymentButtonStyle, PaymentButtonType};

const HELPER_CLASS_NAME: &str = "dev.waterui.payment.PayButtonHelper";

struct ClickPayload {
    active: Cell<bool>,
    action: SharedAction<()>,
    env: Environment,
}

struct CallbackGuard {
    vm: JavaVM,
    helper_class: Global<JClass<'static>>,
    container: Global<JObject<'static>>,
    payload: Rc<ClickPayload>,
    token_addr: usize,
    detached: Cell<bool>,
}

impl CallbackGuard {
    fn detach_with_env(&self, env: &mut Env<'_>) {
        if self.detached.replace(true) {
            return;
        }
        self.payload.active.set(false);
        let _ = env.call_static_method(
            &self.helper_class,
            jni_str!("detachContainer"),
            jni_sig!("(Landroid/view/View;)V"),
            &[JValue::Object(&self.container)],
        );
        let raw_ptr = self.token_addr as *const ClickPayload;
        // SAFETY: `token_addr` was produced by `Rc::into_raw` when creating
        // this view, and `detachContainer` has now cleared `OnClickListener`
        // and zeroed `container.tag` synchronously on the UI thread.
        unsafe {
            drop(Rc::from_raw(raw_ptr));
        }
    }
}

impl Drop for CallbackGuard {
    fn drop(&mut self) {
        if self.detached.get() {
            return;
        }
        let _ = self.vm.attach_current_thread(|env| -> JniResult<()> {
            self.detach_with_env(env);
            Ok(())
        });
    }
}

struct AndroidPaymentContent {
    button_type: Computed<PaymentButtonType>,
    style: Computed<PaymentButtonStyle>,
    corner_radius: Option<f32>,
    allowed_payment_methods: Str,
    action: SharedAction<()>,
    env: Environment,
    accessibility_label: Computed<Str>,
}

pub fn payment_view(button: PaymentButton, env: &Environment) -> impl View {
    let accessibility_label = button.accessibility_label_signal();
    AndroidView::new(AndroidPaymentContent {
        button_type: button.button_type,
        style: button.style,
        corner_radius: button.corner_radius,
        allowed_payment_methods: button.allowed_payment_methods,
        action: button.on_press,
        env: env.clone(),
        accessibility_label,
    })
}

const fn map_theme_ordinal(style: PaymentButtonStyle, color_scheme: ColorScheme) -> i32 {
    match style {
        PaymentButtonStyle::Automatic => match color_scheme {
            ColorScheme::Light => 1,
            ColorScheme::Dark => 0,
        },
        PaymentButtonStyle::Light | PaymentButtonStyle::LightOutline => 0,
        PaymentButtonStyle::Dark => 1,
    }
}

/// Native callback invoked from `dev.waterui.payment.PayButtonHelper.nativeOnClick(long)`.
///
/// # Safety
///
/// Registered through JNI `RegisterNatives` for `(J)V`; `callback_token` must
/// be a token returned from `Arc::into_raw` whose `CallbackGuard` has not yet
/// detached.
unsafe extern "system" fn native_on_click(
    mut env: EnvUnowned<'_>,
    _class: JClass<'_>,
    callback_token: jlong,
) {
    let () = env
        .with_env(|_env| -> JniResult<()> {
            let Ok(addr) = usize::try_from(callback_token) else {
                return Ok(());
            };
            if addr == 0 {
                return Ok(());
            }
            let ptr = addr as *const ClickPayload;
            // SAFETY: `callback_token` remains live until `CallbackGuard` calls
            // `detachContainer` synchronously on the Android UI thread.
            let payload = unsafe { &*ptr };
            if payload.active.get() {
                payload.action.run(&payload.env);
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

fn load_helper_class<'local>(
    env: &mut Env<'local>,
    context: &JObject<'_>,
) -> JniResult<JClass<'local>> {
    let loader = env
        .call_method(
            context,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )?
        .l()?;
    let class_name = env.new_string(HELPER_CLASS_NAME)?;
    let class_obj = env
        .call_method(
            &loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&class_name)],
        )?
        .l()?;
    let helper_class = env.cast_local::<JClass>(class_obj)?;

    let methods = [
        // SAFETY: `native_on_click` matches `(J)V` with `unsafe extern "system"`.
        unsafe {
            NativeMethod::from_raw_parts(
                jni_str!("nativeOnClick"),
                jni_str!("(J)V"),
                native_on_click as *mut c_void,
            )
        },
    ];
    // SAFETY: `helper_class` is the loaded `PayButtonHelper` Java class;
    // `RegisterNatives` is idempotent across mounts.
    unsafe { env.register_native_methods(&helper_class, &methods)? };

    Ok(helper_class)
}

struct ContainerCreationParams<'a> {
    context: &'a JObject<'a>,
    kind: PaymentButtonType,
    style: PaymentButtonStyle,
    color_scheme: ColorScheme,
    corner_radius_dp: f32,
    allowed_methods: &'a str,
    token_jlong: jlong,
    raw_ptr: *const ClickPayload,
}

fn create_container_refs(
    env: &mut Env<'_>,
    helper_class: &JClass<'_>,
    params: &ContainerCreationParams<'_>,
) -> JniResult<(
    Global<JObject<'static>>,
    Global<JObject<'static>>,
    Global<JObject<'static>>,
)> {
    let allowed_jstr = match env.new_string(params.allowed_methods) {
        Ok(value) => value,
        Err(err) => {
            // SAFETY: `createContainer` has not been called yet, so reclaiming `raw_ptr` is exclusive.
            unsafe { drop(Rc::from_raw(params.raw_ptr)) };
            return Err(err);
        }
    };

    let container_local = match env
        .call_static_method(
            helper_class,
            jni_str!("createContainer"),
            jni_sig!("(Landroid/content/Context;IIFLjava/lang/String;J)Landroid/view/View;"),
            &[
                JValue::Object(params.context),
                JValue::Int(params.kind.android_ordinal()),
                JValue::Int(map_theme_ordinal(params.style, params.color_scheme)),
                JValue::Float(params.corner_radius_dp),
                JValue::Object(&allowed_jstr),
                JValue::Long(params.token_jlong),
            ],
        )
        .and_then(jni::JValueOwned::l)
    {
        Ok(view) => view,
        Err(err) => {
            // SAFETY: `createContainer` failed so no Java view retained the token.
            unsafe { drop(Rc::from_raw(params.raw_ptr)) };
            return Err(err);
        }
    };

    match (
        env.new_global_ref(&container_local),
        env.new_global_ref(&container_local),
        env.new_global_ref(&container_local),
    ) {
        (Ok(a), Ok(b), Ok(c)) => Ok((a, b, c)),
        (Err(err), _, _) | (_, Err(err), _) | (_, _, Err(err)) => {
            let _ = env.call_static_method(
                helper_class,
                jni_str!("detachContainer"),
                jni_sig!("(Landroid/view/View;)V"),
                &[JValue::Object(&container_local)],
            );
            // SAFETY: `detachContainer` cleared the Java callback before reclaiming `raw_ptr`.
            unsafe { drop(Rc::from_raw(params.raw_ptr)) };
            Err(err)
        }
    }
}

impl AndroidContent for AndroidPaymentContent {
    fn make_view(
        &mut self,
        env: &mut Env<'_>,
        mount: &mut AndroidMount<'_>,
    ) -> JniResult<Global<JObject<'static>>> {
        let helper_class = load_helper_class(env, mount.context())?;
        let helper_for_guard = env.new_global_ref(&helper_class)?;
        let helper_for_update = env.new_global_ref(&helper_class)?;

        let color_scheme_signal = current_color_scheme(mount.env());
        let corner_radius_dp = self.corner_radius.unwrap_or(-1.0);
        let allowed_methods = self.allowed_payment_methods.clone();

        let payload = Rc::new(ClickPayload {
            active: Cell::new(true),
            action: self.action.clone(),
            env: mount.env().clone(),
        });
        let raw_ptr = Rc::into_raw(Rc::clone(&payload));
        let token_addr = raw_ptr as usize;
        let token_jlong = i64::try_from(token_addr).map_err(|_| {
            // SAFETY: reclaiming the pointer just produced by `Rc::into_raw`.
            unsafe { drop(Rc::from_raw(raw_ptr)) };
            JniError::ParseFailed("pointer does not fit in jlong".into())
        })?;

        let (container_global, container_for_guard, container_for_update) = create_container_refs(
            env,
            &helper_class,
            &ContainerCreationParams {
                context: mount.context(),
                kind: self.button_type.snapshot(),
                style: self.style.snapshot(),
                color_scheme: color_scheme_signal.snapshot(),
                corner_radius_dp,
                allowed_methods: allowed_methods.as_str(),
                token_jlong,
                raw_ptr,
            },
        )?;

        let vm = mount.vm().clone();
        mount.keep(CallbackGuard {
            vm: vm.clone(),
            helper_class: helper_for_guard,
            container: container_for_guard,
            payload,
            token_addr,
            detached: Cell::new(false),
        });

        let invalidator = mount.invalidator();
        let did_mount = Cell::new(false);
        let combined = self
            .button_type
            .clone()
            .zip(&self.style)
            .zip(&color_scheme_signal);

        mount.bind(&combined, move |((kind, style), color_scheme)| {
            if !did_mount.replace(true) {
                return;
            }
            let _ = vm.attach_current_thread(|env| -> JniResult<()> {
                let methods_jstr = env.new_string(allowed_methods.as_str())?;
                env.call_static_method(
                    &helper_for_update,
                    jni_str!("updateContainer"),
                    jni_sig!("(Landroid/view/View;IIFLjava/lang/String;J)V"),
                    &[
                        JValue::Object(&container_for_update),
                        JValue::Int(kind.android_ordinal()),
                        JValue::Int(map_theme_ordinal(style, color_scheme)),
                        JValue::Float(corner_radius_dp),
                        JValue::Object(&methods_jstr),
                        JValue::Long(token_jlong),
                    ],
                )?;
                Ok(())
            });
            invalidator.invalidate_layout();
            invalidator.invalidate_display();
        });

        Ok(container_global)
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
