package com.workiva.frugal.transport;

import io.nats.client.Connection;
import io.nats.client.Connection.Status;
import org.apache.thrift.transport.TTransportException;
import org.junit.Before;
import org.junit.Test;

import static com.workiva.frugal.transport.FNatsTransport.FRUGAL_PREFIX;
import static com.workiva.frugal.transport.FNatsTransport.NATS_MAX_MESSAGE_SIZE;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

/**
 * Tests for {@link FNatsPublisherTransport}.
 */
public class FNatsPublisherTransportTest {

    private FNatsPublisherTransport transport;
    private Connection conn;
    private String topic = "topic";
    private String formattedSubject = FRUGAL_PREFIX + topic;


    @Before
    public void setUp() {
        conn = mock(Connection.class);
        FNatsPublisherTransport.Factory factory = new FNatsPublisherTransport.Factory(conn);
        transport = (FNatsPublisherTransport) factory.getTransport();
    }

    @Test
    public void testOpen() throws TTransportException {
        when(conn.getStatus()).thenReturn(Status.CONNECTED);
        // Verify the connection state is the only open criteria
        assertTrue(transport.isOpen());
        // Verify open doesn't throw TTransportException
        transport.open();
        // Verify closing the transport has no effect
        transport.close();
        assertTrue(transport.isOpen());
    }

    @Test(expected = TTransportException.class)
    public void testNotConnected() throws TTransportException {
        when(conn.getStatus()).thenReturn(Status.DISCONNECTED);
        assertFalse(transport.isOpen());

        // Verify that open throws a TTransportException
        transport.open();
    }

    @Test
    public void testGetPublishSizeLimit() {
        assertEquals(NATS_MAX_MESSAGE_SIZE, transport.getPublishSizeLimit());
    }

    @Test
    public void testPublish() throws Exception {
        when(conn.getStatus()).thenReturn(Status.CONNECTED);
        byte[] payload = new byte[]{1, 2, 3, 4};

        transport.publish(topic, payload);

        verify(conn).publish(formattedSubject, payload);
    }

    @Test(expected = TTransportException.class)
    public void testPublishNotConnected() throws TTransportException {
        when(conn.getStatus()).thenReturn(Status.DISCONNECTED);
        byte[] payload = new byte[]{1, 2, 3, 4};

        transport.publish(topic, payload);
    }

    @Test(expected = TTransportException.class)
    public void testPublishEmptyTopic() throws TTransportException {
        when(conn.getStatus()).thenReturn(Status.CONNECTED);
        String topic = "";
        byte[] payload = new byte[]{1, 2, 3, 4};

        transport.publish(topic, payload);
    }

    @Test(expected = TTransportException.class)
    public void testPublishPayloadTooLarge() throws TTransportException {
        when(conn.getStatus()).thenReturn(Status.CONNECTED);
        byte[] payload = new byte[NATS_MAX_MESSAGE_SIZE + 1];

        transport.publish(topic, payload);
    }
}
