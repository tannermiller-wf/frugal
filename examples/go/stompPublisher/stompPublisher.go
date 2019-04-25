package main

import (
	"fmt"

	"git.apache.org/thrift.git/lib/go/thrift"
	"github.com/go-stomp/stomp"

	"github.com/Workiva/frugal/examples/go/gen-go/v1/music"
	"github.com/Workiva/frugal/lib/go"
)

// Run a Stomp Publisher
func main() {
	fProtocolFactory := frugal.NewFProtocolFactory(thrift.NewTBinaryProtocolFactoryDefault())

	conn, err := stomp.Dial("tcp", "localhost:61613")
	if err != nil {
		panic(err)
	}
	defer conn.Disconnect()

	pubFactory := frugal.NewFStompPublisherTransportFactoryBuilder(conn).Build()
	provider := frugal.NewFScopeProvider(pubFactory, nil, fProtocolFactory)

	publisher := music.NewAlbumWinnersPublisher(provider)

	// Open the publisher to receive traffic
	if err := publisher.Open(); err != nil {
		panic(err)
	}
	defer publisher.Close()

	// Publish an event
	ctx := frugal.NewFContext("a-corr-id")
	album := &music.Album{
		ASIN:     "c54d385a-5024-4f3f-86ef-6314546a7e7f",
		Duration: 1200,
		Tracks: []*music.Track{{
			Title:     "Comme des enfants",
			Artist:    "Coeur de pirate",
			Publisher: "Grosse Boîte",
			Composer:  "Béatrice Martin",
			Duration:  169,
			Pro:       music.PerfRightsOrg_ASCAP,
		}},
	}
	if err := publisher.PublishWinner(ctx, album); err != nil {
		panic(err)
	}
	if err := publisher.PublishContestStart(ctx, []*music.Album{album, album}); err != nil {
		panic(err)
	}

	fmt.Println("WinnerPublished ...")
}
