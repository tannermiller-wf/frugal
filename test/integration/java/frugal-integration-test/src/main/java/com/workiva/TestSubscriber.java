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
import org.apache.thrift.TException;
import org.apache.thrift.protocol.TProtocolFactory;

import javax.jms.Session;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

import static com.workiva.Utils.PREAMBLE_HEADER;
import static com.workiva.Utils.RAMBLE_HEADER;
import static com.workiva.Utils.whichProtocolFactory;

public class TestSubscriber {

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
                ActiveMQConnectionFactory connectionFactory = new ActiveMQConnectionFactory("tcp://localhost:61616");

                // Create a Connection
                javax.jms.Connection connection = connectionFactory.createConnection();
                connection.start();

                publisherFactory = new FJmsPublisherTransport.Factory.Builder(connection).build();
                subscriberFactory = new FJmsSubscriberTransport.Factory.Builder(connection).build();
                break;
            default:
                throw new IllegalArgumentException("Unknown transport type " + transportType);
        }

        FScopeProvider provider = new FScopeProvider(publisherFactory, subscriberFactory, new FProtocolFactory(protocolFactory));
        EventsSubscriber.Iface subscriber = new EventsSubscriber.Client(provider);

        CountDownLatch messageReceived = new CountDownLatch(1);

        subscriber.subscribeEventCreated("*", "*", "call", Integer.toString(port), (context, event) -> {
            System.out.println("received " + context + " : " + event);
            EventsPublisher.Iface publisher = new EventsPublisher.Client(provider);
            try {
                publisher.open();
                String preamble = context.getRequestHeader(PREAMBLE_HEADER);
                if (preamble == null || "".equals(preamble)) {
                    System.out.println("Client did not provide preamble header");
                    return;
                }
                String ramble = context.getRequestHeader(RAMBLE_HEADER);
                if (ramble == null || "".equals(ramble)) {
                    System.out.println("Client did not provide ramble header");
                    return;
                }
                event = new Event(1, "received call");
                publisher.publishEventCreated(new FContext("Call"), preamble, ramble, "response", Integer.toString(port), event);
                messageReceived.countDown();
            } catch (TException e) {
                System.out.println("Error opening publisher to respond" + e.getMessage());
            }
        });

        new com.workiva.HealthCheck(port);

        System.out.println("Subscriber started...");
        if (messageReceived.await(15, TimeUnit.SECONDS)) {
            System.out.println("received message from publisher");
        } else {
            throw new RuntimeException("did not receive message from publisher");
        }

        System.exit(0);
    }
}
