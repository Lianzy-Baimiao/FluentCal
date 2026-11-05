package com.fluentcal.mobile;

import static org.junit.Assert.*;
import java.util.Collections;
import okhttp3.Response;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.Test;

public class SyncHttpClientTest {
    @Test public void onlyAllowsPrivateLiteralOneUsePairingEndpointsOverHttp() throws Exception {
        String suffix = ":18766/pair/" + "a".repeat(32);
        assertTrue(SyncHttpPlugin.isPairRequest(new java.net.URI("http://192.168.1.2" + suffix), "GET"));
        for (String host : new String[] { "127.0.0.1", "8.8.8.8", "169.254.169.254", "192.168.1.999", "010.1.2.3", "localhost" })
            assertFalse(SyncHttpPlugin.isPairRequest(new java.net.URI("http://" + host + suffix), "GET"));
        assertFalse(SyncHttpPlugin.isPairRequest(new java.net.URI("http://192.168.1.2" + suffix), "POST"));
        assertFalse(SyncHttpPlugin.isPairRequest(new java.net.URI("http://192.168.1.2" + suffix + "?token=x"), "GET"));
    }
    @Test public void supportsMkcolAndExactJsonConditionalPut() throws Exception {
        try (MockWebServer server = new MockWebServer()) {
            server.enqueue(new MockResponse().setResponseCode(201));
            server.enqueue(new MockResponse().setResponseCode(204));
            SyncHttpClient client = new SyncHttpClient();
            try (Response r = client.execute(server.url("/日历/").toString(), "MKCOL", Collections.emptyMap(), null)) { assertEquals(201, r.code()); }
            assertEquals("MKCOL", server.takeRequest().getMethod());
            String text = "{\"app\":\"FluentCal\",\"任务\":1}";
            try (Response r = client.execute(server.url("/data.json").toString(), "PUT", Collections.singletonMap("If-Match", "\"v1\""), text)) { assertEquals(204, r.code()); }
            RecordedRequest put = server.takeRequest();
            assertEquals(text, put.getBody().readUtf8());
            assertEquals("\"v1\"", put.getHeader("If-Match"));
        }
    }
    @Test public void doesNotForwardCredentialsAcrossRedirects() throws Exception {
        try (MockWebServer server = new MockWebServer()) {
            server.enqueue(new MockResponse().setResponseCode(302).setHeader("Location", server.url("/other")));
            try (Response r = new SyncHttpClient().execute(server.url("/source").toString(), "GET", Collections.singletonMap("Authorization", "Basic secret"), null)) { assertEquals(302, r.code()); }
            assertEquals(1, server.getRequestCount());
        }
    }
    @Test public void returnsMalformedJsonAsTextForSafeRejectionBySyncEngine() throws Exception {
        try (MockWebServer server = new MockWebServer()) {
            server.enqueue(new MockResponse().setHeader("Content-Type", "application/json").setBody("{broken"));
            try (Response r = new SyncHttpClient().execute(server.url("/").toString(), "GET", Collections.emptyMap(), null)) {
                assertEquals("{broken", SyncHttpClient.readText(r.body(), 100));
            }
        }
    }
    @Test public void boundsChunkedResponseWhileReading() throws Exception {
        try (MockWebServer server = new MockWebServer()) {
            server.enqueue(new MockResponse().setChunkedBody("too much data", 2));
            try (Response r = new SyncHttpClient().execute(server.url("/").toString(), "GET", Collections.emptyMap(), null)) {
                assertThrows(SyncHttpClient.TooLargeException.class, () -> SyncHttpClient.readText(r.body(), 5));
            }
        }
    }
}
