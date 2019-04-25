/*
 * Copyright 2017 Workiva
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package com.workiva.frugal.transport;

import com.workiva.frugal.exception.TTransportExceptionType;
import io.nats.client.Connection;
import io.nats.client.Connection.Status;
import io.nats.client.Dispatcher;
import io.nats.client.Message;
import io.nats.client.MessageHandler;
import org.apache.thrift.TException;
import org.apache.thrift.transport.TTransportException;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Arrays;
import java.util.UUID;

/**
 * FNatsTransport is an extension of FTransport. This is a "stateless" transport
 * in the sense that there is no connection with a server. A request is simply
 * published to a subject and responses are received on another subject. This
 * assumes requests/responses fit within a single NATS message.
 */
public class FNatsTransport extends FAsyncTransport {

    private static final Logger LOGGER = LoggerFactory.getLogger(FNatsTransport.class);

    public static final int NATS_MAX_MESSAGE_SIZE = 1024 * 1024;
    public static final String FRUGAL_PREFIX = "frugal.";

    private final Connection conn;
    private final String subject;
    private final String inbox;
    private Dispatcher dispatcher;

    private FNatsTransport(Connection conn, String subject, String inbox) {
        this.requestSizeLimit = NATS_MAX_MESSAGE_SIZE;
        this.conn = conn;
        this.subject = subject;
        this.inbox = inbox;
    }

    /**
     * Creates a new FTransport which uses the NATS messaging system as the underlying transport.
     * A request is simply published to a subject and responses are received on a randomly generated
     * subject. This requires requests to fit within a single NATS message.
     *
     * This transport uses a randomly generated inbox for receiving NATS replies.
     *
     * @param conn    NATS connection
     * @param subject subject to publish requests on
     * @return FNatsTransport for communicating via NATS.
     */
    public static FNatsTransport of(Connection conn, String subject) {
        return new FNatsTransport(conn, subject, createInbox(conn));
    }

    /**
     * Returns a new FTransport configured with the specified inbox.
     *
     * @param inbox NATS subject to receive responses on
     * @return FNatsTransport for communicating via NATS.
     */
    public FNatsTransport withInbox(String inbox) {
        return new FNatsTransport(conn, subject, inbox);
    }


    /**
     * Query transport open state.
     *
     * @return true if transport and NATS connection are open.
     */
    @Override
    public boolean isOpen() {
        return dispatcher != null && conn.getStatus() == Status.CONNECTED;
    }

    /**
     * Subscribes to the configured inbox subject.
     *
     * @throws TTransportException if unable to open the transport
     */
    @Override
    public void open() throws TTransportException {
        if (conn.getStatus() != Status.CONNECTED) {
            throw getClosedConditionException(conn.getStatus(), "open:");
        }
        if (dispatcher != null) {
            throw new TTransportException(TTransportExceptionType.ALREADY_OPEN, "NATS transport already open");
        }
        dispatcher = conn.createDispatcher(new Handler());
        dispatcher.subscribe(inbox);
    }

    /**
     * Unsubscribes from the inbox subject and closes the response buffer.
     */
    @Override
    public void close() {
        if (dispatcher != null) {
            dispatcher.unsubscribe(subject);
            dispatcher = null;
        }
        super.close();
    }

    @Override
    protected void flush(byte[] payload) throws TTransportException {
        if (!isOpen()) {
            throw getClosedConditionException(conn.getStatus(), "flush:");
        }
        conn.publish(subject, inbox, payload);
    }

    /**
     * NATS message handler that executes Frugal frames.
     */
    protected class Handler implements MessageHandler {
        public void onMessage(Message message) {
            try {
                byte[] frame = message.getData();
                handleResponse(Arrays.copyOfRange(frame, 4, frame.length));
            } catch (TException e) {
                LOGGER.warn("Could not handle frame", e);
            }
        }

    }

    /**
     * Convert NATS connection state to a suitable exception type.
     *
     * @param connStatus nats connection status
     * @param prefix     prefix to add to exception message
     * @return a TTransportException type
     */
    protected static TTransportException getClosedConditionException(Status connStatus, String prefix) {
        if (connStatus != Status.CONNECTED) {
            return new TTransportException(TTransportExceptionType.NOT_OPEN,
                    String.format("%s NATS client not connected (has status %s)", prefix, connStatus.name()));
        }
        return new TTransportException(TTransportExceptionType.NOT_OPEN,
                String.format("%s NATS Transport not open", prefix));
    }

    /**
     * Helper to generate a random id for inbox.
     */
    private static String createInbox(Connection conn) {
        return conn.getOptions().getInboxPrefix() + UUID.randomUUID().toString().replace("-", "");
    }
}
