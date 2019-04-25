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

package common

import (
	"fmt"
	"log"
	"net/http"

	"git.apache.org/thrift.git/lib/go/thrift"

	"github.com/Workiva/frugal/lib/go"
	"github.com/Workiva/frugal/test/integration/go/gen/frugaltest"
)

func StartSubscriber(host string,
	port int64,
	transport string,
	protocol string,
	handler frugaltest.FFrugalTest,
	pubSubResponseSent chan bool) {

	var protocolFactory thrift.TProtocolFactory
	switch protocol {
	case "compact":
		protocolFactory = thrift.NewTCompactProtocolFactory()
	case "json":
		protocolFactory = thrift.NewTJSONProtocolFactory()
	case "binary":
		protocolFactory = thrift.NewTBinaryProtocolFactoryDefault()
	default:
		panic(fmt.Errorf("Invalid protocol specified %s", protocol))
	}

	go func() {
		var pfactory frugal.FPublisherTransportFactory
		var sfactory frugal.FSubscriberTransportFactory

		switch transport {
		case ActiveMqName:
			stompConn := getStompConn()
			pfactory = frugal.NewFStompPublisherTransportFactoryBuilder(stompConn).Build()
			sfactory = frugal.NewFStompSubscriberTransportFactoryBuilder(stompConn).Build()
		default:
			panic(fmt.Errorf("invalid transport specified %s", transport))
		}

		provider := frugal.NewFScopeProvider(pfactory, sfactory, frugal.NewFProtocolFactory(protocolFactory))
		subscriber := frugaltest.NewEventsSubscriber(provider)

		// TODO: Document SubscribeEventCreated "user" cannot contain spaces
		_, err := subscriber.SubscribeEventCreated("*", "*", "call", fmt.Sprintf("%d", port), func(ctx frugal.FContext, e *frugaltest.Event) {
			// Send a message back to the client
			fmt.Printf("received %+v : %+v\n", ctx, e)
			publisher := frugaltest.NewEventsPublisher(provider)
			if err := publisher.Open(); err != nil {
				panic(err)
			}
			defer publisher.Close()
			preamble, ok := ctx.RequestHeader(preambleHeader)
			if !ok {
				log.Fatal("Client did provide a preamble header")
			}
			ramble, ok := ctx.RequestHeader(rambleHeader)
			if !ok {
				log.Fatal("Client did provide a ramble header")
			}

			ctx = frugal.NewFContext("Response")
			event := &frugaltest.Event{Message: "received call"}
			if err := publisher.PublishEventCreated(ctx, preamble, ramble, "response", fmt.Sprintf("%d", port), event); err != nil {
				panic(err)
			}

			pubSubResponseSent <- true
		})
		if err != nil {
			panic(err)
		}
	}()

	hostPort := fmt.Sprintf("%s:%d", host, port)
	// Start http server
	// Healthcheck used in the cross language runner to check for server availability
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {})
	go http.ListenAndServe(hostPort, nil)
	fmt.Printf("Starting %v subscriber...\n", transport)
}
