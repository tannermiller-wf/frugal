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
	"time"

	"git.apache.org/thrift.git/lib/go/thrift"

	"github.com/Workiva/frugal/lib/go"
	"github.com/Workiva/frugal/test/integration/go/gen/frugaltest"
)

func StartPublisher(
	host string,
	port int64,
	transport string,
	protocol string,
	pubSub chan bool,
	sent chan bool) error {

	var protocolFactory thrift.TProtocolFactory
	switch protocol {
	case "compact":
		protocolFactory = thrift.NewTCompactProtocolFactory()
	case "json":
		protocolFactory = thrift.NewTJSONProtocolFactory()
	case "binary":
		protocolFactory = thrift.NewTBinaryProtocolFactoryDefault()
	default:
		return fmt.Errorf("invalid protocol specified %s", protocol)
	}

	go func() {
		<-pubSub
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
		publisher := frugaltest.NewEventsPublisher(provider)

		if err := publisher.Open(); err != nil {
			panic(err)
		}
		defer publisher.Close()

		// Start Subscription, pass timeout
		resp := make(chan bool)
		subscriber := frugaltest.NewEventsSubscriber(provider)
		preamble := "foo"
		ramble := "bar"
		// TODO: Document SubscribeEventCreated "user" cannot contain spaces
		_, err := subscriber.SubscribeEventCreated(preamble, ramble, "response", fmt.Sprintf("%d", port), func(ctx frugal.FContext, e *frugaltest.Event) {
			fmt.Printf(" Response received %v\n", e)
			close(resp)
		})
		if err != nil {
			panic(err)
		}
		ctx := frugal.NewFContext("Call")
		ctx.AddRequestHeader(preambleHeader, preamble)
		ctx.AddRequestHeader(rambleHeader, ramble)
		event := &frugaltest.Event{Message: "Sending call"}
		fmt.Print("Publishing... ")
		if err := publisher.PublishEventCreated(ctx, preamble, ramble, "call", fmt.Sprintf("%d", port), event); err != nil {
			panic(err)
		}

		timeout := time.After(time.Second * 3)

		select {
		case <-resp: // Response received is logged in the subscribe
		case <-timeout:
			log.Fatal("Pub/Sub response timed out!")
		}
		close(sent)
	}()

	return nil
}
