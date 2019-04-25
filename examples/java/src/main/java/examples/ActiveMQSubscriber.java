package examples;

import com.workiva.frugal.protocol.FProtocolFactory;
import com.workiva.frugal.provider.FScopeProvider;
import com.workiva.frugal.transport.FJmsSubscriberTransport;
import com.workiva.frugal.transport.FSubscriberTransportFactory;
import org.apache.activemq.ActiveMQConnectionFactory;
import org.apache.thrift.protocol.TBinaryProtocol;
import v1.music.AlbumWinnersSubscriber;

import javax.jms.Connection;

public class ActiveMQSubscriber {
    public static void main(String[] args) throws Exception {
        FProtocolFactory protocolFactory = new FProtocolFactory(new TBinaryProtocol.Factory());

        ActiveMQConnectionFactory connectionFactory = new ActiveMQConnectionFactory();
        Connection connection = connectionFactory.createConnection();
        connection.start();

        FSubscriberTransportFactory subscriberFactory = new FJmsSubscriberTransport.Factory.Builder(connection).build();
        FScopeProvider provider = new FScopeProvider(null, subscriberFactory, protocolFactory);

        AlbumWinnersSubscriber.Iface subscriber = new AlbumWinnersSubscriber.Client(provider);
        subscriber.subscribeWinner((ctx, album) -> System.out.println("You won! " + album));
        subscriber.subscribeContestStart((ctx, albums) -> System.out.println("Contest started, available albums: " + albums));
        System.out.println("Subscriber started...");
    }
}
