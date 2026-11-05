package com.fluentcal.mobile;

import com.getcapacitor.BridgeActivity;
import android.os.Bundle;

public class MainActivity extends BridgeActivity {
    @Override
    public void onCreate(Bundle savedInstanceState) {
        registerPlugin(SyncVaultPlugin.class);
        registerPlugin(SyncHttpPlugin.class);
        registerPlugin(PairScannerPlugin.class);
        super.onCreate(savedInstanceState);
    }
}
