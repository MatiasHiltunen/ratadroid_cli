package com.ratadroid.{name};

import android.os.Bundle;
import android.view.View;
import android.view.inputmethod.InputMethodManager;
import android.content.Context;

public class NativeActivity extends android.app.NativeActivity {
    static {
        System.loadLibrary("ratadroid");
    }

    // NativeActivity will automatically call ANativeActivity_onCreate from the native library
    // No need to override onCreate or call native methods manually
    
    // Method to show soft keyboard - called from native code via JNI
    public void showSoftKeyboard() {
        android.util.Log.d("NativeActivity", "showSoftKeyboard() called from native code");
        InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
        if (imm != null) {
            View view = getWindow().getDecorView();
            if (view != null) {
                view.requestFocus();
                boolean result = imm.showSoftInput(view, InputMethodManager.SHOW_IMPLICIT);
                android.util.Log.d("NativeActivity", "showSoftInput returned: " + result);
            } else {
                android.util.Log.e("NativeActivity", "DecorView is null");
            }
        } else {
            android.util.Log.e("NativeActivity", "InputMethodManager is null");
        }
    }
}

