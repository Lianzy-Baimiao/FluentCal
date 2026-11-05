package com.fluentcal.mobile;

import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import java.net.URI;
import java.util.HashMap;
import java.util.Iterator;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import okhttp3.Response;

@CapacitorPlugin(name = "SyncHttp")
public class SyncHttpPlugin extends Plugin {
    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private final SyncHttpClient client = new SyncHttpClient();

    @PluginMethod
    public void request(PluginCall call) {
        executor.execute(() -> {
            try {
                String url = call.getString("url", "");
                URI uri = new URI(url);
                String method = call.getString("method", "GET");
                boolean pair = isPairRequest(uri, method);
                if ((!"https".equals(uri.getScheme()) && !pair) || uri.getUserInfo() != null || uri.getHost() == null ||
                    !(method.equals("GET") || method.equals("PUT") || method.equals("POST") || method.equals("MKCOL"))) {
                    call.reject("Invalid HTTPS request", "CONFIG"); return;
                }
                Map<String, String> headers = new HashMap<>();
                JSObject input = call.getObject("headers", new JSObject());
                for (Iterator<String> i = input.keys(); i.hasNext();) {
                    String name = i.next(); headers.put(name, input.getString(name));
                }
                if (pair) headers.clear();
                try (Response response = client.execute(url, method, headers, pair ? null : call.getString("body"))) {
                    JSObject result = new JSObject();
                    result.put("status", response.code());
                    JSObject responseHeaders = new JSObject();
                    for (String name : response.headers().names()) responseHeaders.put(name.toLowerCase(Locale.ROOT), response.header(name));
                    result.put("headers", responseHeaders);
                    result.put("text", SyncHttpClient.readText(response.body(), pair ? 20_000 : 30_000_000));
                    call.resolve(result);
                }
            } catch (SyncHttpClient.TooLargeException e) { call.reject("Response too large", "TOO_LARGE"); }
              catch (java.nio.charset.CharacterCodingException e) { call.reject("Invalid UTF-8 response", "BAD_ENCODING"); }
              catch (Exception e) { call.reject("Sync connection failed", "NETWORK"); }
        });
    }
    static boolean isPairRequest(URI uri, String method) {
        String host = uri.getHost();
        if (!"http".equals(uri.getScheme()) || !"GET".equals(method) || host == null || !host.matches("(?:[0-9]{1,3}\\.){3}[0-9]{1,3}") ||
            uri.getUserInfo() != null || uri.getQuery() != null || uri.getFragment() != null || uri.getPort() < 1024 || uri.getPort() > 65535 ||
            !uri.getPath().matches("/pair/[a-f0-9]{32}")) return false;
        String[] parts = host.split("\\.");
        int[] ip = new int[4];
        for (int i = 0; i < 4; i++) { ip[i] = Integer.parseInt(parts[i]); if (ip[i] > 255 || !String.valueOf(ip[i]).equals(parts[i])) return false; }
        return ip[0] == 10 || (ip[0] == 192 && ip[1] == 168) || (ip[0] == 172 && ip[1] >= 16 && ip[1] <= 31);
    }
    @Override
    protected void handleOnDestroy() { executor.shutdownNow(); }
}
