package com.zelenka.music;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.view.View;

import java.util.Random;

/** A lightweight decoration confined to page content; it does not intercept touches. */
public final class SnowView extends View {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final float[] x = new float[48], y = new float[48], speed = new float[48], radius = new float[48];
    private long previousFrame;
    private boolean enabled;

    public SnowView(Context context) {
        super(context);
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);
        setClickable(false);
        Random random = new Random(7);
        for (int i = 0; i < x.length; i++) {
            x[i] = random.nextFloat(); y[i] = random.nextFloat();
            speed[i] = 0.03f + random.nextFloat() * 0.07f;
            radius[i] = 1f + random.nextFloat() * 2f;
        }
    }
    public void setEnabledSnow(boolean value) {
        enabled = value;
        previousFrame = 0;
        setVisibility(value ? VISIBLE : GONE);
        if (enabled) postInvalidateOnAnimation();
    }
    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (!enabled || !isShown()) return;
        long now = android.os.SystemClock.uptimeMillis();
        float elapsed = previousFrame == 0 ? 0 : Math.min(0.05f, (now - previousFrame) / 1000f);
        previousFrame = now;
        paint.setColor(0x85e9fff0);
        float density = getResources().getDisplayMetrics().density;
        for (int i = 0; i < x.length; i++) {
            y[i] = (y[i] + speed[i] * elapsed) % 1f;
            canvas.drawCircle((x[i] + 0.012f * (float) Math.sin(now / 1600f + i)) * getWidth(),
                    y[i] * getHeight(), radius[i] * density, paint);
        }
        postInvalidateDelayed(33);
    }
}
