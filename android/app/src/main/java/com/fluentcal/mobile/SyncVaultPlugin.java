package com.fluentcal.mobile;

import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.AtomicFile;
import android.util.Base64;
import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.security.KeyStore;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;
import org.json.JSONObject;

/** Secrets never enter Capacitor Preferences, exports or Android Auto Backup. */
@CapacitorPlugin(name = "SyncVault")
public class SyncVaultPlugin extends Plugin {
    private static final String ALIAS = "fluentcal.sync.v1";

    private SecretKey key() throws Exception {
        KeyStore store = KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        if (!store.containsAlias(ALIAS)) {
            KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
            generator.init(new KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build());
            generator.generateKey();
        }
        return (SecretKey) store.getKey(ALIAS, null);
    }

    private AtomicFile file(PluginCall call) {
        String name = call.getString("key", "");
        if (!name.equals("webdav") && !name.equals("github") && !name.equals("gitee"))
            throw new IllegalArgumentException("Invalid provider");
        return new AtomicFile(new File(getContext().getNoBackupFilesDir(), "sync-" + name + ".enc"));
    }

    @PluginMethod
    public synchronized void get(PluginCall call) {
        try {
            AtomicFile file = file(call);
            String value = "";
            if (file.getBaseFile().exists()) {
                JSONObject encrypted = new JSONObject(new String(file.readFully(), StandardCharsets.UTF_8));
                Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
                cipher.init(Cipher.DECRYPT_MODE, key(), new GCMParameterSpec(128, Base64.decode(encrypted.getString("iv"), Base64.NO_WRAP)));
                cipher.updateAAD(call.getString("key", "").getBytes(StandardCharsets.UTF_8));
                value = new String(cipher.doFinal(Base64.decode(encrypted.getString("data"), Base64.NO_WRAP)), StandardCharsets.UTF_8);
            }
            JSObject result = new JSObject();
            result.put("value", value);
            call.resolve(result);
        } catch (Exception e) { call.reject("Cannot read sync credential"); }
    }

    @PluginMethod
    public synchronized void set(PluginCall call) {
        FileOutputStream stream = null;
        AtomicFile file = null;
        try {
            file = file(call);
            String value = call.getString("value", "");
            if (value.isEmpty()) { file.delete(); call.resolve(); return; }
            Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            cipher.init(Cipher.ENCRYPT_MODE, key());
            cipher.updateAAD(call.getString("key", "").getBytes(StandardCharsets.UTF_8));
            byte[] data = cipher.doFinal(value.getBytes(StandardCharsets.UTF_8));
            JSONObject encrypted = new JSONObject();
            encrypted.put("iv", Base64.encodeToString(cipher.getIV(), Base64.NO_WRAP));
            encrypted.put("data", Base64.encodeToString(data, Base64.NO_WRAP));
            stream = file.startWrite();
            stream.write(encrypted.toString().getBytes(StandardCharsets.UTF_8));
            file.finishWrite(stream);
            call.resolve();
        } catch (Exception e) {
            if (file != null && stream != null) file.failWrite(stream);
            call.reject("Cannot save sync credential");
        }
    }
}
