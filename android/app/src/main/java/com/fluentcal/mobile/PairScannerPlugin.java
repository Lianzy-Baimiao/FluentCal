package com.fluentcal.mobile;

import androidx.activity.result.ActivityResult;
import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.ActivityCallback;
import com.getcapacitor.annotation.CapacitorPlugin;
import com.journeyapps.barcodescanner.ScanOptions;
import com.journeyapps.barcodescanner.ScanIntentResult;

@CapacitorPlugin(name = "PairScanner")
public class PairScannerPlugin extends Plugin {
    @PluginMethod public void scan(PluginCall call) {
        ScanOptions options = new ScanOptions();
        options.setDesiredBarcodeFormats(ScanOptions.QR_CODE);
        options.setPrompt("扫描电脑 FluentCal「同步」页的配对二维码");
        options.setBeepEnabled(false);
        options.setBarcodeImageEnabled(false);
        options.setOrientationLocked(false);
        startActivityForResult(call, options.createScanIntent(getContext()), "scanned");
    }
    @ActivityCallback private void scanned(PluginCall call, ActivityResult result) {
        if (call == null) return;
        if (result.getData() != null && result.getData().getBooleanExtra("MISSING_CAMERA_PERMISSION", false)) {
            call.reject("Camera permission denied"); return;
        }
        ScanIntentResult scan = ScanIntentResult.parseActivityResult(result.getResultCode(), result.getData());
        JSObject response = new JSObject();
        response.put("text", scan.getContents() == null ? org.json.JSONObject.NULL : scan.getContents());
        call.resolve(response);
    }
}
