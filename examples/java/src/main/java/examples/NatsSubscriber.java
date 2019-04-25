package examples;

import com.workiva.frugal.protocol.FProtocolFactory;
import com.workiva.frugal.provider.FScopeProvider;
import com.workiva.frugal.transport.FNatsPublisherTransport;
import com.workiva.frugal.transport.FNatsSubscriberTransport;
import com.workiva.frugal.transport.FPublisherTransportFactory;
import com.workiva.frugal.transport.FSubscriberTransportFactory;
import v1.music.AlbumWinnersSubscriber;

import io.nats.client.Connection;
import io.nats.client.Nats;
import io.nats.client.Options;
import org.apache.thrift.TException;
import org.apache.thrift.protocol.TBinaryProtocol;

import java.io.IOException;

/**
 * Create a NATS PubSub subscriber.
 */
public class NatsSubscriber {

    public static void main(String[] args) throws TException, IOException, InterruptedException {
        // Specify the protocol used for serializing requests.
        // The protocol stack must match the protocol stack of the publisher.
        FProtocolFactory protocolFactory = new FProtocolFactory(new TBinaryProtocol.Factory());

        // Create a NATS client (using default options for local dev)
        Options.Builder optionsBuilder = new Options.Builder().server(Options.DEFAULT_URL);
        Connection conn = Nats.connect(optionsBuilder.build());

        // Create the pubsub scope provider, given the NATs connection and protocol
        FPublisherTransportFactory publisherFactory = new FNatsPublisherTransport.Factory(conn);
        FSubscriberTransportFactory subscriberFactory = new FNatsSubscriberTransport.Factory(conn);
        FScopeProvider provider = new FScopeProvider(publisherFactory, subscriberFactory, protocolFactory);

        // Subscribe to winner announcements
        AlbumWinnersSubscriber.Iface subscriber = new AlbumWinnersSubscriber.Client(provider);
        subscriber.subscribeWinner((ctx, album) -> System.out.println("You won! " + album));
        subscriber.subscribeContestStart((ctx, albums) -> System.out.println("Contest started, available albums: " + albums));
        System.out.println("Subscriber started...");
    }
}
