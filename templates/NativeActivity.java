package com.ratadroid.{name};

import android.os.Bundle;

public class NativeActivity extends android.app.NativeActivity {
    static {
        System.loadLibrary("ratadroid");
    }

    // NativeActivity will automatically call ANativeActivity_onCreate from the native library
    // No need to override onCreate or call native methods manually
}

