package com.fluentcal.mobile;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.ResponseBody;

/** Supports WebDAV MKCOL, bounded raw text and conditional writes without JSON coercion. */
final class SyncHttpClient {
    private final OkHttpClient client = new OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS).readTimeout(30, TimeUnit.SECONDS)
        .callTimeout(45, TimeUnit.SECONDS).followRedirects(false).followSslRedirects(false)
        .retryOnConnectionFailure(false).build();

    Response execute(String url, String method, Map<String, String> headers, String body) throws IOException {
        Request.Builder request = new Request.Builder().url(url);
        for (Map.Entry<String, String> header : headers.entrySet()) request.header(header.getKey(), header.getValue());
        RequestBody content = body != null ? RequestBody.create(body, MediaType.parse("application/json; charset=utf-8"))
            : (method.equals("MKCOL") || method.equals("POST") || method.equals("PUT")) ? RequestBody.create(new byte[0], null) : null;
        return client.newCall(request.method(method, content).build()).execute();
    }

    static String readText(ResponseBody body, int maxBytes) throws IOException {
        if (body == null) return "";
        if (body.contentLength() > maxBytes) throw new TooLargeException();
        try (InputStream input = body.byteStream(); ByteArrayOutputStream output = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[8192];
            int read;
            while ((read = input.read(buffer)) != -1) {
                if (output.size() + read > maxBytes) throw new TooLargeException();
                output.write(buffer, 0, read);
            }
            return StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(output.toByteArray())).toString();
        }
    }
    static final class TooLargeException extends IOException {}
}
