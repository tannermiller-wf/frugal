package examples;

import com.workiva.frugal.FContext;
import com.workiva.frugal.protocol.FProtocolFactory;
import com.workiva.frugal.provider.FScopeProvider;
import com.workiva.frugal.transport.FNatsPublisherTransport;
import com.workiva.frugal.transport.FNatsSubscriberTransport;
import com.workiva.frugal.transport.FPublisherTransportFactory;
import com.workiva.frugal.transport.FSubscriberTransportFactory;
import v1.music.Album;
import v1.music.AlbumWinnersPublisher;
import v1.music.PerfRightsOrg;
import v1.music.Track;

import io.nats.client.Connection;
import io.nats.client.Nats;
import io.nats.client.Options;
import org.apache.thrift.TException;
import org.apache.thrift.protocol.TBinaryProtocol;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

/**
 * Create a NATS PubSub publisher.
 */
public class NatsPublisher {

    public static void main(String[] args) throws TException, IOException, InterruptedException {
        // Specify the protocol used for serializing requests.
        // The protocol stack must match the protocol stack of the subscriber.
        FProtocolFactory protocolFactory = new FProtocolFactory(new TBinaryProtocol.Factory());

        // Create a NATS client (using default options for local dev)
        Options.Builder optionsBuilder = new Options.Builder().server(Options.DEFAULT_URL);
        Connection conn = Nats.connect(optionsBuilder.build());

        // Create the pubsub scope provider, given the NATs connection and protocol
        FPublisherTransportFactory publisherFactory = new FNatsPublisherTransport.Factory(conn);
        FSubscriberTransportFactory subscriberFactory = new FNatsSubscriberTransport.Factory(conn);
        FScopeProvider provider = new FScopeProvider(publisherFactory, subscriberFactory, protocolFactory);

        // Create and open a publisher
        AlbumWinnersPublisher.Iface publisher = new AlbumWinnersPublisher.Client(provider);
        publisher.open();

        // Publish a winner announcement
        Album album = new Album();
        album.setASIN(UUID.randomUUID().toString());
        album.setDuration(1200);
        album.addToTracks(
                new Track(
                        "Comme des enfants",
                        "Coeur de pirate",
                        "Grosse Boîte",
                        "Béatrice Martin",
                        169,
                        PerfRightsOrg.ASCAP));
        publisher.publishWinner(new FContext(), album);
        List<Album> albums = new ArrayList<>();
        albums.add(album);
        albums.add(album);
        publisher.publishContestStart(new FContext(), albums);

        System.out.println("Published event");

        publisher.close();
        conn.close();
    }
}
