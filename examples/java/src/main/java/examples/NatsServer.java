package examples;

import com.workiva.frugal.protocol.FProtocolFactory;
import com.workiva.frugal.server.FNatsServer;
import com.workiva.frugal.server.FServer;
import v1.music.FStore;

import io.nats.client.Connection;
import io.nats.client.Nats;
import io.nats.client.Options;
import org.apache.thrift.TException;
import org.apache.thrift.protocol.TBinaryProtocol;

import java.io.IOException;

/**
 * Creates a NATS server listening for incoming requests.
 */
public class NatsServer {
    public static final String SERVICE_SUBJECT = "music-service";

    public static void main(String[] args) throws IOException, InterruptedException, TException {
        // Specify the protocol used for serializing requests.
        // Clients must use the same protocol stack
        FProtocolFactory protocolFactory = new FProtocolFactory(new TBinaryProtocol.Factory());

        // Create a NATS client (using default options for local dev)
        Options.Builder optionsBuilder = new Options.Builder().server(Options.DEFAULT_URL);
        Connection conn = Nats.connect(optionsBuilder.build());

        // Create a new server processor.
        // Incoming requests to the server are passed to the processor.
        // Results from the processor are returned back to the client.
        FStore.Processor processor = new FStore.Processor(new FStoreHandler(), new LoggingMiddleware());

        // Create a new music store server using the processor
        // The server can be configured using the Builder interface.
        FServer server =
                new FNatsServer.Builder(conn, processor, protocolFactory, new String[]{SERVICE_SUBJECT})
                        .withQueueGroup(SERVICE_SUBJECT) // if set, all servers listen to the same queue group
                        .build();

        System.out.println("Starting nats server on " + SERVICE_SUBJECT);
        server.serve();
    }

}