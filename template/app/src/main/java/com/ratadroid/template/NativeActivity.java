package com.ratadroid.template;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Typeface;
import android.os.Bundle;

/**
 * Base NativeActivity for Ratadroid TUI applications.
 * 
 * This is a minimal NativeActivity that delegates all functionality to native Rust code.
 * All keyboard handling, rendering, and input processing is done in Rust.
 */
public class NativeActivity extends android.app.NativeActivity {
    static {
        System.loadLibrary("ratadroid");
    }
    
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        
        // Restore state if available
        if (savedInstanceState != null) {
            byte[] savedState = savedInstanceState.getByteArray("native_state");
            if (savedState != null) {
                restoreNativeState(savedState);
            }
        }
    }
    
    @Override
    protected void onSaveInstanceState(Bundle outState) {
        super.onSaveInstanceState(outState);
        
        // Save native state
        byte[] nativeState = saveNativeState();
        if (nativeState != null) {
            outState.putByteArray("native_state", nativeState);
        }
    }
    
    @Override
    protected void onRestoreInstanceState(Bundle savedInstanceState) {
        super.onRestoreInstanceState(savedInstanceState);
        
        // Restore native state
        if (savedInstanceState != null) {
            byte[] savedState = savedInstanceState.getByteArray("native_state");
            if (savedState != null) {
                restoreNativeState(savedState);
            }
        }
    }
    
    @Override
    protected void onDestroy() {
        super.onDestroy();
        // Cleanup is handled by native code
        android.util.Log.i("NativeActivity", "onDestroy called");
    }
    
    // Native state save/restore callbacks
    private native byte[] saveNativeState();
    private native void restoreNativeState(byte[] state);
    
    // Paint objects for text rendering (cached for performance)
    private Paint textPaint;
    private Paint emojiPaint;
    
    /**
     * Render a character using Android's native Canvas/Bitmap APIs.
     * This provides superior emoji rendering quality compared to cosmic-text.
     * 
     * @param character The character to render (as String to support emojis)
     * @param size Font size in pixels
     * @param color ARGB color (0xAARRGGBB format)
     * @return Byte array: [width(4), height(4), isWide(1), ...rgba_pixels] or empty array on failure
     */
    public byte[] renderCharacter(String character, float size, int color) {
        try {
            // Initialize paints if needed
            if (textPaint == null) {
                textPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
                textPaint.setTypeface(Typeface.MONOSPACE);
                textPaint.setTextAlign(Paint.Align.LEFT);
            }
            
            if (emojiPaint == null) {
                emojiPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
                emojiPaint.setTypeface(Typeface.DEFAULT); // Better emoji support
                emojiPaint.setTextAlign(Paint.Align.LEFT);
            }
            
            // Determine if this is an emoji or special character
            boolean isEmoji = isEmojiOrSpecial(character);
            boolean isWide = isWideCharacter(character);
            
            // Select appropriate paint
            Paint paint = isEmoji ? emojiPaint : textPaint;
            // Scale down emoji size slightly (0.9x) to match cosmic-text behavior and fit better in grid
            float renderSize = isEmoji ? size * 0.9f : size;
            paint.setTextSize(renderSize);
            paint.setColor(color);
            
            // Calculate cell dimensions
            float baseCellWidth = (float) Math.ceil(size * 0.6);
            float cellWidth = isWide ? baseCellWidth * 2 : baseCellWidth;
            float cellHeight = (float) Math.ceil(size);
            
            // Ensure minimum dimensions
            int bitmapWidth = Math.max((int) cellWidth, 4);
            int bitmapHeight = Math.max((int) cellHeight, 4);
            
            // Measure text bounds
            Paint.FontMetrics fontMetrics = paint.getFontMetrics();
            float textWidth = paint.measureText(character);
            float textHeight = fontMetrics.descent - fontMetrics.ascent;
            
            // Create bitmap with ARGB_8888 format
            Bitmap bitmap = Bitmap.createBitmap(bitmapWidth, bitmapHeight, Bitmap.Config.ARGB_8888);
            Canvas canvas = new Canvas(bitmap);
            
            // Clear to transparent
            canvas.drawColor(0x00000000);
            
            // Calculate text position (centered both horizontally and vertically)
            // For horizontal centering: center x = (bitmapWidth - textWidth) / 2
            float x = (bitmapWidth - textWidth) / 2.0f;
            
            // For vertical centering:
            // drawText uses baseline y coordinate. The text extends from:
            //   top = baseline + ascent (ascent is negative, so this is above baseline)
            //   bottom = baseline + descent (descent is positive, so this is below baseline)
            // To center: the midpoint (baseline + (ascent + descent)/2) should be at bitmapHeight/2
            // Therefore: baseline = bitmapHeight/2 - (ascent + descent)/2
            float y = bitmapHeight / 2.0f - (fontMetrics.ascent + fontMetrics.descent) / 2.0f;
            
            // Draw text
            canvas.drawText(character, x, y, paint);
            
            // Extract pixel data
            int[] pixels = new int[bitmapWidth * bitmapHeight];
            bitmap.getPixels(pixels, 0, bitmapWidth, 0, 0, bitmapWidth, bitmapHeight);
            
            // Convert ARGB to RGBA and create output array
            // Format: [width(4), height(4), isWide(1), ...rgba_pixels]
            byte[] result = new byte[4 + 4 + 1 + (bitmapWidth * bitmapHeight * 4)];
            
            // Write width (4 bytes, little-endian)
            result[0] = (byte) (bitmapWidth & 0xFF);
            result[1] = (byte) ((bitmapWidth >> 8) & 0xFF);
            result[2] = (byte) ((bitmapWidth >> 16) & 0xFF);
            result[3] = (byte) ((bitmapWidth >> 24) & 0xFF);
            
            // Write height (4 bytes, little-endian)
            result[4] = (byte) (bitmapHeight & 0xFF);
            result[5] = (byte) ((bitmapHeight >> 8) & 0xFF);
            result[6] = (byte) ((bitmapHeight >> 16) & 0xFF);
            result[7] = (byte) ((bitmapHeight >> 24) & 0xFF);
            
            // Write isWide flag (1 byte)
            result[8] = (byte) (isWide ? 1 : 0);
            
            // Convert ARGB pixels to RGBA bytes
            int offset = 9;
            for (int pixel : pixels) {
                int a = (pixel >> 24) & 0xFF;
                int r = (pixel >> 16) & 0xFF;
                int g = (pixel >> 8) & 0xFF;
                int b = pixel & 0xFF;
                
                result[offset++] = (byte) r;
                result[offset++] = (byte) g;
                result[offset++] = (byte) b;
                result[offset++] = (byte) a;
            }
            
            bitmap.recycle();
            return result;
            
        } catch (Exception e) {
            android.util.Log.e("NativeActivity", "Error rendering character: " + e.getMessage(), e);
            return new byte[0];
        }
    }
    
    /**
     * Check if a character is an emoji or special symbol.
     * Matches the Rust implementation in rasterizer.rs
     */
    private boolean isEmojiOrSpecial(String character) {
        if (character == null || character.isEmpty()) {
            return false;
        }
        int codePoint = character.codePointAt(0);
        return (codePoint >= 0x1F300 && codePoint <= 0x1F9FF) || // Misc Symbols, Emoticons
               (codePoint >= 0x1FA00 && codePoint <= 0x1FAFF) || // Extended symbols
               (codePoint >= 0x2600 && codePoint <= 0x26FF) ||   // Misc symbols
               (codePoint >= 0x2700 && codePoint <= 0x27BF) ||   // Dingbats
               (codePoint >= 0x1F600 && codePoint <= 0x1F64F) || // Emoticons
               (codePoint >= 0x1F900 && codePoint <= 0x1F9FF) || // Supplemental Symbols and Pictographs
               (codePoint >= 0x1F1E0 && codePoint <= 0x1F1FF);    // Regional Indicator Symbols (Flags)
    }
    
    /**
     * Check if a character is wide (takes 2 terminal cells).
     * Matches the Rust implementation in rasterizer.rs
     */
    private boolean isWideCharacter(String character) {
        if (character == null || character.isEmpty()) {
            return false;
        }
        int codePoint = character.codePointAt(0);
        
        // Check for emojis (wide)
        if (isEmojiOrSpecial(character)) {
            return true;
        }
        
        // Check for CJK characters and other wide characters
        return (codePoint >= 0x1100 && codePoint <= 0x115F) || // Hangul Jamo
               (codePoint >= 0x2E80 && codePoint <= 0x2EFF) || // CJK Radicals Supplement
               (codePoint >= 0x2F00 && codePoint <= 0x2FDF) || // Kangxi Radicals
               (codePoint >= 0x3000 && codePoint <= 0x303F) || // CJK Symbols and Punctuation
               (codePoint >= 0x3040 && codePoint <= 0x309F) || // Hiragana
               (codePoint >= 0x30A0 && codePoint <= 0x30FF) || // Katakana
               (codePoint >= 0x3100 && codePoint <= 0x312F) || // Bopomofo
               (codePoint >= 0x3130 && codePoint <= 0x318F) || // Hangul Compatibility Jamo
               (codePoint >= 0x3200 && codePoint <= 0x32FF) || // Enclosed CJK Letters and Months
               (codePoint >= 0x3300 && codePoint <= 0x33FF) || // CJK Compatibility
               (codePoint >= 0x3400 && codePoint <= 0x4DBF) || // CJK Unified Ideographs Extension A
               (codePoint >= 0x4E00 && codePoint <= 0x9FFF) || // CJK Unified Ideographs
               (codePoint >= 0xA000 && codePoint <= 0xA48F) || // Yi Syllables
               (codePoint >= 0xA490 && codePoint <= 0xA4CF) || // Yi Radicals
               (codePoint >= 0xAC00 && codePoint <= 0xD7AF) || // Hangul Syllables
               (codePoint >= 0xF900 && codePoint <= 0xFAFF) || // CJK Compatibility Ideographs
               (codePoint >= 0xFE30 && codePoint <= 0xFE4F) || // CJK Compatibility Forms
               (codePoint >= 0xFF00 && codePoint <= 0xFFEF);   // Halfwidth and Fullwidth Forms
    }
}
