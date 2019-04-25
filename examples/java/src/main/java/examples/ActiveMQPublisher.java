package examples;

import com.workiva.frugal.FContext;
import com.workiva.frugal.protocol.FProtocolFactory;
import com.workiva.frugal.provider.FScopeProvider;
import com.workiva.frugal.transport.FJmsPublisherTransport;
import com.workiva.frugal.transport.FPublisherTransportFactory;
import org.apache.activemq.ActiveMQConnectionFactory;
import org.apache.thrift.protocol.TBinaryProtocol;
import v1.music.Album;
import v1.music.AlbumWinnersPublisher;
import v1.music.PerfRightsOrg;
import v1.music.Track;

import javax.jms.Connection;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

public class ActiveMQPublisher {
    public static void main(String[] args) throws Exception {
        FProtocolFactory protocolFactory = new FProtocolFactory(new TBinaryProtocol.Factory());

        ActiveMQConnectionFactory connectionFactory = new ActiveMQConnectionFactory();
        Connection connection = connectionFactory.createConnection();
        connection.start();

        FPublisherTransportFactory publisherFactory = new FJmsPublisherTransport.Factory.Builder(connection).build();

        FScopeProvider provider = new FScopeProvider(publisherFactory, null, protocolFactory);

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
        connection.close();
    }
}
