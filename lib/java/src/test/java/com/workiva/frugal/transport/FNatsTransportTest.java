package com.workiva.frugal.transport;

import com.workiva.frugal.FContext;
import com.workiva.frugal.exception.TTransportExceptionType;
import io.nats.client.Connection;
import io.nats.client.Connection.Status;
import io.nats.client.Dispatcher;
import io.nats.client.Message;
import io.nats.client.MessageHandler;
import io.nats.client.Options;
import org.apache.thrift.TException;
import org.apache.thrift.transport.TTransportException;
import org.junit.Before;
import org.junit.Test;
import org.mockito.ArgumentCaptor;

import java.io.IOException;
import java.util.concurrent.BlockingQueue;

import static com.workiva.frugal.transport.FAsyncTransportTest.mockFrame;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.fail;
import static org.mockito.Mockito.any;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

/**
 * Tests for {@link FNatsTransport}.
 */
public class FNatsTransportTest {

    private Connection conn;
    private String subject = "foo";
    private String inbox = "bar";
    private FNatsTransport transport;

    @Before
    public void setUp() {
        conn = mock(Connection.class);
        when(conn.getOptions()).thenReturn(new Options.Builder().build());
        transport = FNatsTransport.of(conn, subject).withInbox(inbox);
    }

    @Test(expected = TTransportException.class)
    public void testOpenNatsDisconnected() throws TTransportException {
        assertFalse(transport.isOpen());
        when(conn.getStatus()).thenReturn(Status.CLOSED);
        transport.open();
    }

    @Test
    public void testOpenCallbackClose() throws TException, IOException, InterruptedException {
        assertFalse(transport.isOpen());
        when(conn.getStatus()).thenReturn(Status.CONNECTED);
        ArgumentCaptor<String> inboxCaptor = ArgumentCaptor.forClass(String.class);
        ArgumentCaptor<MessageHandler> handlerCaptor = ArgumentCaptor.forClass(MessageHandler.class);
        Dispatcher mockDispatcher = mock(Dispatcher.class);
        when(conn.createDispatcher(handlerCaptor.capture())).thenReturn(mockDispatcher);

        transport.open();

        verify(mockDispatcher).subscribe(inboxCaptor.capture());
        assertEquals(inbox, inboxCaptor.getValue());

        MessageHandler handler = handlerCaptor.getValue();
        FContext context = new FContext();
        BlockingQueue<byte[]> mockQueue = mock(BlockingQueue.class);
        transport.queueMap.put(FAsyncTransport.getOpId(context), mockQueue);

        byte[] mockFrame = mockFrame(context);
        byte[] framedPayload = new byte[mockFrame.length + 4];
        System.arraycopy(mockFrame, 0, framedPayload, 4, mockFrame.length);

        Message mockMessage = mock(Message.class);
        when(mockMessage.getData()).thenReturn(framedPayload);
        handler.onMessage(mockMessage);

        try {
            transport.open();
            fail("Expected TTransportException");
        } catch (TTransportException e) {
            assertEquals(TTransportExceptionType.ALREADY_OPEN, e.getType());
        }

        FTransportClosedCallback mockCallback = mock(FTransportClosedCallback.class);
        transport.setClosedCallback(mockCallback);
        transport.close();

        verify(mockDispatcher).unsubscribe(subject);
        verify(mockCallback).onClose(null);
        verify(mockQueue).put(mockFrame);
    }

    @Test
    public void testFlush() throws TTransportException {
        when(conn.getStatus()).thenReturn(Status.CONNECTED);
        Dispatcher mockDispatcher = mock(Dispatcher.class);
        when(conn.createDispatcher(any(MessageHandler.class))).thenReturn(mockDispatcher);
        transport.open();

        byte[] buff = "helloworld".getBytes();
        transport.flush(buff);
        verify(conn).publish(subject, inbox, buff);
    }

    @Test(expected = TTransportException.class)
    public void testRequestNotOpen() throws TTransportException {
        when(conn.getStatus()).thenReturn(Status.CONNECTED);
        transport.request(new FContext(), "helloworld".getBytes());
    }
}
