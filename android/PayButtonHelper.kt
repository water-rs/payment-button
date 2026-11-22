package dev.waterui.payment

import android.content.Context
import android.view.View
import android.view.ViewGroup
import android.widget.FrameLayout
import com.google.android.gms.wallet.button.ButtonConstants
import com.google.android.gms.wallet.button.ButtonOptions
import com.google.android.gms.wallet.button.PayButton
import java.util.concurrent.atomic.AtomicLong
import kotlin.math.roundToInt

/**
 * Kotlin bridge creating and updating Google Pay's native [PayButton] for
 * `waterui-payment-button`'s `AndroidContent` implementation.
 */
object PayButtonHelper {
    @JvmStatic
    fun createContainer(
        context: Context,
        buttonTypeOrdinal: Int,
        buttonThemeOrdinal: Int,
        cornerRadiusDp: Float,
        allowedPaymentMethods: String,
        callbackToken: Long,
    ): View {
        val container = FrameLayout(context)
        container.tag = AtomicLong(callbackToken)
        installPayButton(
            container = container,
            buttonTypeOrdinal = buttonTypeOrdinal,
            buttonThemeOrdinal = buttonThemeOrdinal,
            cornerRadiusDp = cornerRadiusDp,
            allowedPaymentMethods = allowedPaymentMethods,
        )
        return container
    }

    @JvmStatic
    fun updateContainer(
        view: View,
        buttonTypeOrdinal: Int,
        buttonThemeOrdinal: Int,
        cornerRadiusDp: Float,
        allowedPaymentMethods: String,
        callbackToken: Long,
    ) {
        val container = view as FrameLayout
        val tokenRef = container.tag as? AtomicLong
        if (tokenRef != null) {
            tokenRef.set(callbackToken)
        } else {
            container.tag = AtomicLong(callbackToken)
        }
        installPayButton(
            container = container,
            buttonTypeOrdinal = buttonTypeOrdinal,
            buttonThemeOrdinal = buttonThemeOrdinal,
            cornerRadiusDp = cornerRadiusDp,
            allowedPaymentMethods = allowedPaymentMethods,
        )
    }

    @JvmStatic
    fun detachContainer(view: View) {
        val container = view as FrameLayout
        (container.tag as? AtomicLong)?.set(0L)
        for (index in 0 until container.childCount) {
            container.getChildAt(index).setOnClickListener(null)
        }
        container.removeAllViews()
    }

    private fun installPayButton(
        container: FrameLayout,
        buttonTypeOrdinal: Int,
        buttonThemeOrdinal: Int,
        cornerRadiusDp: Float,
        allowedPaymentMethods: String,
    ) {
        for (index in 0 until container.childCount) {
            container.getChildAt(index).setOnClickListener(null)
        }
        container.removeAllViews()

        val context = container.context
        val payButton = PayButton(context)
        val builder = ButtonOptions.newBuilder()
            .setButtonType(mapButtonType(buttonTypeOrdinal))
            .setButtonTheme(mapButtonTheme(buttonThemeOrdinal))
            .setAllowedPaymentMethods(allowedPaymentMethods)
        if (cornerRadiusDp >= 0f) {
            val radiusPx = (cornerRadiusDp * context.resources.displayMetrics.density).roundToInt()
            builder.setCornerRadius(radiusPx)
        }
        payButton.initialize(builder.build())
        payButton.setOnClickListener {
            val token = (container.tag as? AtomicLong)?.get() ?: 0L
            if (token != 0L) {
                nativeOnClick(token)
            }
        }
        container.addView(
            payButton,
            FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
            ),
        )
    }

    private fun mapButtonType(ordinal: Int): Int = when (ordinal) {
        0 -> ButtonConstants.ButtonType.PLAIN
        1 -> ButtonConstants.ButtonType.BUY
        2 -> ButtonConstants.ButtonType.PAY
        3 -> ButtonConstants.ButtonType.CHECKOUT
        4 -> ButtonConstants.ButtonType.BOOK
        5 -> ButtonConstants.ButtonType.SUBSCRIBE
        6 -> ButtonConstants.ButtonType.DONATE
        7 -> ButtonConstants.ButtonType.ORDER
        else -> throw IllegalArgumentException("unknown PaymentButtonType ordinal: $ordinal")
    }

    private fun mapButtonTheme(ordinal: Int): Int = when (ordinal) {
        0 -> ButtonConstants.ButtonTheme.LIGHT
        1 -> ButtonConstants.ButtonTheme.DARK
        else -> throw IllegalArgumentException("unknown PaymentButtonTheme ordinal: $ordinal")
    }

    @JvmStatic
    external fun nativeOnClick(callbackToken: Long)
}
