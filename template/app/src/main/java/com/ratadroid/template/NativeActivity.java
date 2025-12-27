package com.ratadroid.template;

import android.os.Bundle;
import android.view.View;
import android.view.ViewTreeObserver;
import android.view.inputmethod.InputMethodManager;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.Typeface;
import android.graphics.Color;

/**
 * Base NativeActivity for Ratadroid TUI applications.
 * 
 * This activity provides:
 * - Keyboard visibility detection and notification to Rust
 * - Screen insets (status bar, navigation bar) reporting
 * - Character rendering via Android's Canvas/TextPaint APIs
 * - Soft keyboard control
 * 
 * To use in your app, you can either:
 * 1. Use this class directly in AndroidManifest.xml
 * 2. Extend this class and override methods as needed
 */
public class NativeActivity extends android.app.NativeActivity {
    static {
        // Load the native library - override this in your app's NativeActivity
        // if you use a different library name
        System.loadLibrary("ratadroid");
    }

    // Reusable rendering objects for performance
    private Paint mTextPaint;
    private Paint mEmojiPaint;
    private float mLastTextSize = -1;
    private int mRenderCount = 0;
    private static final int LOG_INTERVAL = 100;
    
    // Keyboard visibility tracking
    private int mLastVisibleHeight = -1;
    private boolean mKeyboardVisible = false;
    
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        
        // Set up a layout listener to detect keyboard visibility changes
        final View rootView = getWindow().getDecorView().getRootView();
        rootView.getViewTreeObserver().addOnGlobalLayoutListener(new ViewTreeObserver.OnGlobalLayoutListener() {
            @Override
            public void onGlobalLayout() {
                Rect r = new Rect();
                rootView.getWindowVisibleDisplayFrame(r);
                int visibleHeight = r.height();
                
                // Only trigger if height actually changed significantly
                if (mLastVisibleHeight != -1 && Math.abs(visibleHeight - mLastVisibleHeight) > 100) {
                    boolean wasKeyboardVisible = mKeyboardVisible;
                    
                    int screenHeight = rootView.getHeight();
                    int heightDiff = screenHeight - visibleHeight;
                    
                    // If height difference is more than 15% of screen, keyboard is probably visible
                    mKeyboardVisible = heightDiff > (screenHeight * 0.15);
                    
                    if (wasKeyboardVisible != mKeyboardVisible) {
                        android.util.Log.i("NativeActivity", "Keyboard visibility changed: " + 
                            (mKeyboardVisible ? "SHOWN" : "HIDDEN") + 
                            " (visibleHeight=" + visibleHeight + ", screenHeight=" + screenHeight + ")");
                        
                        notifyKeyboardVisibilityChanged(mKeyboardVisible, visibleHeight);
                    }
                }
                
                mLastVisibleHeight = visibleHeight;
            }
        });
    }
    
    // Native method to notify Rust code of keyboard visibility changes
    private native void notifyKeyboardVisibilityChanged(boolean visible, int visibleHeight);

    // Method to show soft keyboard - called from native code via JNI
    // Must be run on UI thread for IMM methods to work correctly
    public void showSoftKeyboard() {
        android.util.Log.d("NativeActivity", "showSoftKeyboard() called from native code");
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
                    if (imm != null) {
                        View view = getWindow().getDecorView();
                        if (view != null) {
                            view.requestFocus();
                            boolean result = imm.showSoftInput(view, InputMethodManager.SHOW_IMPLICIT);
                            android.util.Log.d("NativeActivity", "showSoftInput returned: " + result);
                            if (!result) {
                                // Try the toggle method as fallback
                                imm.toggleSoftInput(InputMethodManager.SHOW_FORCED, 0);
                            }
                        }
                    }
                } catch (Exception e) {
                    android.util.Log.e("NativeActivity", "Error showing keyboard: " + e.getMessage());
                }
            }
        });
    }
    
    // Method to hide soft keyboard - called from native code via JNI
    // Must be run on UI thread for IMM methods to work correctly
    public void hideSoftKeyboard() {
        android.util.Log.d("NativeActivity", "hideSoftKeyboard() called from native code");
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
                    if (imm != null) {
                        View view = getWindow().getDecorView();
                        if (view != null) {
                            imm.hideSoftInputFromWindow(view.getWindowToken(), 0);
                            android.util.Log.d("NativeActivity", "hideSoftInputFromWindow called");
                        }
                    }
                } catch (Exception e) {
                    android.util.Log.e("NativeActivity", "Error hiding keyboard: " + e.getMessage());
                }
            }
        });
    }
    
    // Get the navigation bar height in pixels
    public int getNavigationBarHeight() {
        int resourceId = getResources().getIdentifier("navigation_bar_height", "dimen", "android");
        if (resourceId > 0) {
            return getResources().getDimensionPixelSize(resourceId);
        }
        return 0;
    }
    
    // Get the status bar height in pixels
    public int getStatusBarHeight() {
        int resourceId = getResources().getIdentifier("status_bar_height", "dimen", "android");
        if (resourceId > 0) {
            return getResources().getDimensionPixelSize(resourceId);
        }
        return 0;
    }
    
    // Get screen insets as array [top, bottom, left, right] in pixels
    public int[] getScreenInsets() {
        int[] insets = new int[4];
        
        try {
            View decorView = getWindow().getDecorView();
            if (decorView != null && android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.M) {
                android.view.WindowInsets windowInsets = decorView.getRootWindowInsets();
                if (windowInsets != null) {
                    if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                        android.graphics.Insets systemBars = windowInsets.getInsets(
                            android.view.WindowInsets.Type.systemBars()
                        );
                        insets[0] = systemBars.top;
                        insets[1] = systemBars.bottom;
                        insets[2] = systemBars.left;
                        insets[3] = systemBars.right;
                    } else {
                        insets[0] = windowInsets.getSystemWindowInsetTop();
                        insets[1] = windowInsets.getSystemWindowInsetBottom();
                        insets[2] = windowInsets.getSystemWindowInsetLeft();
                        insets[3] = windowInsets.getSystemWindowInsetRight();
                    }
                }
            }
        } catch (Exception e) {
            android.util.Log.e("NativeActivity", "Failed to get screen insets: " + e.getMessage());
            insets[0] = getStatusBarHeight();
            insets[1] = getNavigationBarHeight();
        }
        
        return insets;
    }
    
    // Initialize or update Paint objects when size changes
    private void ensurePaints(float size) {
        if (mTextPaint == null || mLastTextSize != size) {
            mTextPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
            mTextPaint.setTextSize(size);
            mTextPaint.setTypeface(Typeface.MONOSPACE);
            mTextPaint.setSubpixelText(true);
            
            mEmojiPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
            mEmojiPaint.setTextSize(size);
            mEmojiPaint.setTypeface(Typeface.DEFAULT);
            mEmojiPaint.setSubpixelText(true);
            
            mLastTextSize = size;
        }
    }
    
    // Check if character is an emoji or special Unicode
    private boolean isEmojiOrSpecial(String character) {
        if (character == null || character.isEmpty()) return false;
        int codePoint = character.codePointAt(0);
        return (codePoint >= 0x1F300 && codePoint <= 0x1F9FF) ||
               (codePoint >= 0x1FA00 && codePoint <= 0x1FAFF) ||
               (codePoint >= 0x2600 && codePoint <= 0x26FF) ||
               (codePoint >= 0x2700 && codePoint <= 0x27BF) ||
               (codePoint >= 0x1F600 && codePoint <= 0x1F64F) ||
               (codePoint >= 0x1F900 && codePoint <= 0x1F9FF) ||
               (codePoint >= 0x1F1E0 && codePoint <= 0x1F1FF);
    }
    
    // Check if character is a wide character (CJK, etc.)
    private boolean isWideCharacter(String character) {
        if (character == null || character.isEmpty()) return false;
        int codePoint = character.codePointAt(0);
        return (codePoint >= 0x1100 && codePoint <= 0x115F) ||
               (codePoint >= 0x2E80 && codePoint <= 0x9FFF) ||
               (codePoint >= 0xAC00 && codePoint <= 0xD7A3) ||
               (codePoint >= 0xF900 && codePoint <= 0xFAFF) ||
               (codePoint >= 0xFE10 && codePoint <= 0xFE1F) ||
               (codePoint >= 0xFF00 && codePoint <= 0xFFEF) ||
               isEmojiOrSpecial(character);
    }
    
    // Render a character using Android's native TextPaint/Canvas
    // Returns byte array: [width (4 bytes), height (4 bytes), isWide (1 byte), ...rgba_pixels]
    public byte[] renderCharacter(String character, float size, int color) {
        mRenderCount++;
        
        try {
            ensurePaints(size);
            
            Paint paint = isEmojiOrSpecial(character) ? mEmojiPaint : mTextPaint;
            paint.setColor(color);
            
            Paint.FontMetrics fontMetrics = paint.getFontMetrics();
            float textWidth = paint.measureText(character);
            float textHeight = Math.abs(fontMetrics.bottom - fontMetrics.top);
            
            int padding = (int)(size * 0.1f);
            int bitmapWidth = Math.max((int)Math.ceil(textWidth) + padding * 2, 1);
            int bitmapHeight = Math.max((int)Math.ceil(textHeight) + padding * 2, 1);
            
            Bitmap bitmap = Bitmap.createBitmap(bitmapWidth, bitmapHeight, Bitmap.Config.ARGB_8888);
            Canvas canvas = new Canvas(bitmap);
            canvas.drawColor(Color.TRANSPARENT, android.graphics.PorterDuff.Mode.CLEAR);
            
            float x = padding;
            float y = padding - fontMetrics.top;
            canvas.drawText(character, x, y, paint);
            
            int[] pixels = new int[bitmapWidth * bitmapHeight];
            bitmap.getPixels(pixels, 0, bitmapWidth, 0, 0, bitmapWidth, bitmapHeight);
            bitmap.recycle();
            
            boolean isWide = isWideCharacter(character);
            
            byte[] result = new byte[9 + pixels.length * 4];
            
            // Write width (little-endian)
            result[0] = (byte)(bitmapWidth & 0xFF);
            result[1] = (byte)((bitmapWidth >> 8) & 0xFF);
            result[2] = (byte)((bitmapWidth >> 16) & 0xFF);
            result[3] = (byte)((bitmapWidth >> 24) & 0xFF);
            
            // Write height (little-endian)
            result[4] = (byte)(bitmapHeight & 0xFF);
            result[5] = (byte)((bitmapHeight >> 8) & 0xFF);
            result[6] = (byte)((bitmapHeight >> 16) & 0xFF);
            result[7] = (byte)((bitmapHeight >> 24) & 0xFF);
            
            // Write isWide flag
            result[8] = (byte)(isWide ? 1 : 0);
            
            // Write RGBA pixels
            int offset = 9;
            for (int pixel : pixels) {
                result[offset++] = (byte)((pixel >> 16) & 0xFF); // R
                result[offset++] = (byte)((pixel >> 8) & 0xFF);  // G
                result[offset++] = (byte)(pixel & 0xFF);         // B
                result[offset++] = (byte)((pixel >> 24) & 0xFF); // A
            }
            
            return result;
            
        } catch (Exception e) {
            android.util.Log.e("NativeActivity", "renderCharacter failed: " + e.getMessage(), e);
            return new byte[0];
        }
    }
}

