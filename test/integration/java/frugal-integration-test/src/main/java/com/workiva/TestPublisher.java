package com.workiva;


import com.workiva.frugal.FContext;
import com.workiva.frugal.protocol.FProtocolFactory;
import com.workiva.frugal.provider.FScopeProvider;
import com.workiva.frugal.transport.FJmsPublisherTransport;
import com.workiva.frugal.transport.FJmsSubscriberTransport;
import com.workiva.frugal.transport.FPublisherTransportFactory;
import com.workiva.frugal.transport.FSubscriberTransportFactory;
import frugal.test.Event;
import frugal.test.EventsPublisher;
import frugal.test.EventsSubscriber;
import org.apache.activemq.ActiveMQConnectionFactory;
import org.apache.thrift.protocol.TProtocolFactory;

import javax.jms.Connection;
import javax.jms.Session;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

import static com.workiva.Utils.whichProtocolFactory;

public class TestPublisher {
    public static void main(String[] args) throws Exception {
        CrossTestsArgParser parser = new CrossTestsArgParser(args);
        String host = parser.getHost();
        int port = parser.getPort();
        String protocolType = parser.getProtocolType();
        String transportType = parser.getTransportType();

        TProtocolFactory protocolFactory = whichProtocolFactory(protocolType);
        FPublisherTransportFactory publisherFactory;
        FSubscriberTransportFactory subscriberFactory;

        switch (transportType) {
            case Utils.activemqName:
                // TODO
                ActiveMQConnectionFactory connectionFactory = new ActiveMQConnectionFactory("tcp://localhost:61616");

                // Create a Connection
                Connection connection = connectionFactory.createConnection();
                connection.start();

                publisherFactory = new FJmsPublisherTransport.Factory.Builder(connection).build();
                subscriberFactory = new FJmsSubscriberTransport.Factory.Builder(connection).build();
                break;
            default:
                throw new IllegalArgumentException("Unknown transport type " + transportType);
        }


        /*
         * PUB/SUB TEST
         * Publish a message, verify that a subscriber receives the message and publishes a response.
         * Verifies that scopes are correctly generated.
         */
        CountDownLatch messageReceived = new CountDownLatch(1);
        FScopeProvider provider = new FScopeProvider(publisherFactory, subscriberFactory, new FProtocolFactory(protocolFactory));

        String preamble = "foo";
        String ramble = "bar";
        EventsSubscriber.Iface subscriber = new EventsSubscriber.Client(provider);
        subscriber.subscribeEventCreated(preamble, ramble, "response", Integer.toString(port), (ctx, event) -> {
            System.out.println("Response received " + event);
            messageReceived.countDown();
        });

        EventsPublisher.Iface publisher = new EventsPublisher.Client(provider);
        publisher.open();
        Event event = new Event(1, "Sending Call");
        FContext ctx = new FContext("Call");
        ctx.addRequestHeader(Utils.PREAMBLE_HEADER, preamble);
        ctx.addRequestHeader(Utils.RAMBLE_HEADER, ramble);
        publisher.publishEventCreated(ctx, preamble, ramble, "call", Integer.toString(port), event);
        System.out.print("Publishing...    ");

        if (messageReceived.await(15, TimeUnit.SECONDS)) {
            System.out.println("received message from subscriber");
        } else {
            throw new RuntimeException("did not receive message from subscriber");
        }

        System.exit(0);
    }
}