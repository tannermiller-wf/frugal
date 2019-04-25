package com.workiva.frugal.transport;

import com.workiva.frugal.FContext;
import com.workiva.frugal.exception.TTransportExceptionType;
import com.workiva.frugal.transport.FHttpTransport.FHttpTransportHeaders;

import org.apache.commons.codec.binary.Base64;
import org.apache.http.Header;
import org.apache.http.HttpStatus;
import org.apache.http.HttpVersion;
import org.apache.http.NoHttpResponseException;
import org.apache.http.ProtocolVersion;
import org.apache.http.StatusLine;
import org.apache.http.client.methods.CloseableHttpResponse;
import org.apache.http.client.methods.HttpPost;
import org.apache.http.entity.ContentType;
import org.apache.http.entity.StringEntity;
import org.apache.http.impl.client.CloseableHttpClient;
import org.apache.http.message.BasicHeader;
import org.apache.http.message.BasicHttpResponse;
import org.apache.http.util.EntityUtils;
import org.apache.thrift.TException;
import org.apache.thrift.transport.TTransport;
import org.apache.thrift.transport.TTransportException;
import org.junit.Before;
import org.junit.Test;
import org.mockito.ArgumentCaptor;

import java.io.IOException;
import java.net.SocketTimeoutException;
import java.util.HashMap;
import java.util.Map;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

/**
 * Tests for {@link FHttpTransport}.
 */
public class FHttpTransportTest {

    private CloseableHttpClient client;
    private String url = "http://foo.com";
    private FHttpTransport transport;
    private FContext context = new FContext();

    @Before
    public void setUp() {
        client = mock(CloseableHttpClient.class);
        transport = new FHttpTransport.Builder(client, url).build();
    }

    @Test
    public void testOpenClose() throws TTransportException, IOException, InterruptedException {
        assertTrue(transport.isOpen());
        transport.open();
        assertTrue(transport.isOpen());
        transport.close();
        assertTrue(transport.isOpen());
    }

    @Test(expected = TTransportException.class)
    public void testRequestSizeException() throws TTransportException {
        int requestSizeLimit = 1024 * 4;
        transport = new FHttpTransport.Builder(client, url).withRequestSizeLimit(requestSizeLimit).build();
        transport.request(context, new byte[requestSizeLimit + 1]);
    }

    @Test(expected = TTransportException.class)
    public void testOnewaySizeException() throws TTransportException {
        int requestSizeLimit = 1024 * 4;
        transport = new FHttpTransport.Builder(client, url).withRequestSizeLimit(requestSizeLimit).build();
        transport.oneway(context, new byte[requestSizeLimit + 1]);
    }

    @Test
    public void testRequest() throws TException, IOException {
        int responseSizeLimit = 1024 * 4;
        transport = new FHttpTransport.Builder(client, url).withResponseSizeLimit(responseSizeLimit).build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] framedResponsePayload = new byte[]{0, 1, 2, 3, 4, 5, 6, 7};
        byte[] responsePayload = new byte[]{4, 5, 6, 7};
        String encoded = Base64.encodeBase64String(framedResponsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        TTransport actualResponse = transport.request(context, buff);

        assertArrayEquals(responsePayload, actualResponse.getBuffer());

        HttpPost actual = topicCaptor.getValue();
        HttpPost expected = validRequest(buff, responseSizeLimit);
        assertEquals(EntityUtils.toString(expected.getEntity()), EntityUtils.toString(actual.getEntity()));
        assertEquals(expected.getFirstHeader("content-type"), actual.getFirstHeader("content-type"));
        assertEquals(expected.getURI(), actual.getURI());
    }

    private class TestFHttpTransportHeaders implements FHttpTransportHeaders {
        public Map<String, String> getRequestHeaders() {
            Map<String, String> ret = new HashMap<String, String>();
            ret.put("foo", "bar");
            return ret;
        }
    }

    private class EmptyFHttpTransportHeaders implements FHttpTransportHeaders {
        public Map<String, String> getRequestHeaders() {
            Map<String, String> ret = new HashMap<String, String>();
            return ret;
        }
    }

    @Test
    public void testRequestHeaders() throws TException, IOException {
        FHttpTransportHeaders requestHeaders = new TestFHttpTransportHeaders();
        transport = new FHttpTransport.Builder(client, url)
                .withRequestHeaders(requestHeaders)
                .build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] framedResponsePayload = new byte[]{0, 1, 2, 3, 4, 5, 6, 7};
        String encoded = Base64.encodeBase64String(framedResponsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);

        Header expected = new BasicHeader("foo", "bar");
        Header actual = topicCaptor.getValue().getHeaders("foo")[0];
        assertEquals(expected.getName(), actual.getName());
        assertEquals(expected.getValue(), actual.getValue());
    }

    @Test
    public void testEmptyRequestHeaders() throws TException, IOException {
        FHttpTransportHeaders requestHeaders = new EmptyFHttpTransportHeaders();
        transport = new FHttpTransport.Builder(client, url)
                .withRequestHeaders(requestHeaders)
                .build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] framedResponsePayload = new byte[]{0, 1, 2, 3, 4, 5, 6, 7};
        String encoded = Base64.encodeBase64String(framedResponsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);

        Header[] actual = topicCaptor.getValue().getHeaders("foo");
        assertEquals(0, actual.length);
    }

    @Test
    public void testNullRequestHeaders() throws TException, IOException {
        transport = new FHttpTransport.Builder(client, url)
                .withRequestHeaders(null)
                .build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] framedResponsePayload = new byte[]{0, 1, 2, 3, 4, 5, 6, 7};
        String encoded = Base64.encodeBase64String(framedResponsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);

        Header[] actual = topicCaptor.getValue().getHeaders("foo");
        assertEquals(0, actual.length);
    }

    private class TestContextFHttpTransportHeaders implements FHttpTransportHeaders {
        public Map<String, String> getRequestHeaders() {
            throw new UnsupportedOperationException();
        }

        public Map<String, String> getRequestHeaders(FContext context) {
            Map<String, String> ret = new HashMap<String, String>();
            ret.put("foo", "bar");
            return ret;
        }
    }

    private class EmptyContextFHttpTransportHeaders implements FHttpTransportHeaders {
        public Map<String, String> getRequestHeaders() {
            throw new UnsupportedOperationException();
        }

        public Map<String, String> getRequestHeaders(FContext context) {
            Map<String, String> ret = new HashMap<String, String>();
            return ret;
        }
    }

    @Test
    public void testContextRequestHeaders() throws TException, IOException {
        FHttpTransportHeaders requestHeaders = new TestContextFHttpTransportHeaders();
        transport = new FHttpTransport.Builder(client, url)
                .withRequestHeaders(requestHeaders)
                .build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] framedResponsePayload = new byte[]{0, 1, 2, 3, 4, 5, 6, 7};
        String encoded = Base64.encodeBase64String(framedResponsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);

        Header expected = new BasicHeader("foo", "bar");
        Header actual = topicCaptor.getValue().getHeaders("foo")[0];
        assertEquals(expected.getName(), actual.getName());
        assertEquals(expected.getValue(), actual.getValue());
    }

    @Test
    public void testEmptyContextRequestHeaders() throws TException, IOException {
        FHttpTransportHeaders requestHeaders = new EmptyContextFHttpTransportHeaders();
        transport = new FHttpTransport.Builder(client, url)
                .withRequestHeaders(requestHeaders)
                .build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] framedResponsePayload = new byte[]{0, 1, 2, 3, 4, 5, 6, 7};
        String encoded = Base64.encodeBase64String(framedResponsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);

        Header[] actual = topicCaptor.getValue().getHeaders("foo");
        assertEquals(0, actual.length);
    }

    @Test
    public void testOneway() throws TException, IOException {
        transport = new FHttpTransport.Builder(client, url).build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] framedResponsePayload = new byte[]{0, 1, 2, 3, 4, 5, 6, 7};
        String encoded = Base64.encodeBase64String(framedResponsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.oneway(context, buff);

        HttpPost actual = topicCaptor.getValue();
        HttpPost expected = validRequest(buff, 0);
        assertEquals(EntityUtils.toString(expected.getEntity()), EntityUtils.toString(actual.getEntity()));
        assertEquals(expected.getFirstHeader("content-type"), actual.getFirstHeader("content-type"));
        assertEquals(expected.getURI(), actual.getURI());
    }

    @Test
    public void testSend_requestIOException() throws TTransportException, IOException {
        byte[] buff = "helloserver".getBytes();
        when(client.execute(any(HttpPost.class))).thenThrow(new IOException());
        try {
            transport.request(context, buff);
            fail();
        } catch (TTransportException e) {
            assertEquals(e.getType(), TTransportException.UNKNOWN);
        }
    }

    @Test
    public void testSend_requestTimeoutException() throws TTransportException, IOException {
        byte[] buff = "helloserver".getBytes();
        when(client.execute(any(HttpPost.class))).thenThrow(new SocketTimeoutException());
        try {
            transport.request(context, buff);
            fail();
        } catch (TTransportException e) {
            assertEquals(e.getType(), TTransportException.TIMED_OUT);
        }
    }

    @Test
    public void testSend_requestNoResponse() throws TTransportException, IOException {
        byte[] buff = "helloserver".getBytes();
        when(client.execute(any(HttpPost.class))).thenThrow(new NoHttpResponseException("test"));
        try {
            transport.request(context, buff);
            fail();
        } catch (TTransportException e) {
            assertEquals(e.getType(), TTransportExceptionType.END_OF_FILE);
        }
    }

    @Test(expected = TTransportException.class)
    public void testSend_responseTooLarge() throws TTransportException, IOException {
        int responseSizeLimit = 1024 * 4;
        transport = new FHttpTransport.Builder(client, url).withResponseSizeLimit(responseSizeLimit).build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_REQUEST_TOO_LONG, null);
        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);
    }

    @Test(expected = TTransportException.class)
    public void testSend_responseBadStatus() throws TTransportException, IOException {
        transport = new FHttpTransport.Builder(client, url).build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_BAD_REQUEST, null);
        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);
    }

    @Test(expected = TTransportException.class)
    public void testSend_badResponseLength() throws TTransportException, IOException {
        transport = new FHttpTransport.Builder(client, url).build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] responsePayload = new byte[1];
        String encoded = Base64.encodeBase64String(responsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);
    }

    @Test(expected = TTransportException.class)
    public void testSend_missingData() throws TTransportException, IOException {
        transport = new FHttpTransport.Builder(client, url).build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] responsePayload = new byte[] {(byte) 0x00, (byte) 0x00, (byte) 0x00, (byte) 0x01};
        String encoded = Base64.encodeBase64String(responsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        transport.request(context, buff);
    }

    @Test
    public void testSend_oneWay() throws TException, IOException {
        transport = new FHttpTransport.Builder(client, url).build();

        StatusLine statusLine = new StatusLineImpl(HttpVersion.HTTP_1_1, HttpStatus.SC_OK, null);
        byte[] responsePayload = new byte[] {(byte) 0x00, (byte) 0x00, (byte) 0x00, (byte) 0x00};
        String encoded = Base64.encodeBase64String(responsePayload);
        StringEntity responseEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        CloseableHttpResponse response = new BasicClosableHttpResponse(statusLine);
        response.setEntity(responseEntity);

        ArgumentCaptor<HttpPost> topicCaptor = ArgumentCaptor.forClass(HttpPost.class);
        when(client.execute(topicCaptor.capture())).thenReturn(response);

        byte[] buff = "helloserver".getBytes();
        assertNull(transport.request(context, buff));
    }

    private HttpPost validRequest(byte[] payload, int responseSizeLimit) {
        // Encode request payload
        String encoded = Base64.encodeBase64String(payload);
        StringEntity requestEntity = new StringEntity(encoded, ContentType.create("application/x-frugal", "utf-8"));

        // Set headers and payload
        HttpPost request = new HttpPost(url);
        request.setHeader("accept", "application/x-frugal");
        request.setHeader("content-transfer-encoding", "base64");
        if (responseSizeLimit > 0) {
            request.setHeader("x-frugal-payload-limit", Integer.toString(responseSizeLimit));
        }
        request.setEntity(requestEntity);
        return request;
    }

    private class StatusLineImpl implements StatusLine {
        private ProtocolVersion protocolVersion;
        private int statusCode;
        private String reasonPhrase;

        StatusLineImpl(ProtocolVersion protocolVersion, int statusCode, String reasonPhrase) {
            this.protocolVersion = protocolVersion;
            this.statusCode = statusCode;
            this.reasonPhrase = reasonPhrase;
        }

        @Override
        public ProtocolVersion getProtocolVersion() {
            return protocolVersion;
        }

        @Override
        public int getStatusCode() {
            return statusCode;
        }

        @Override
        public String getReasonPhrase() {
            return reasonPhrase;
        }
    }

    private class BasicClosableHttpResponse extends BasicHttpResponse implements CloseableHttpResponse {

        BasicClosableHttpResponse(StatusLine statusline) {
            super(statusline);
        }

        public void close() throws IOException { }
    }
}
