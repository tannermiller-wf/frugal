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

package frugal

import (
	"bytes"
	"fmt"
	"sync"

	"git.apache.org/thrift.git/lib/go/thrift"
	"github.com/go-stomp/stomp"
)


type FStompPublisherTransportFactoryBuilder struct {
	conn *stomp.Conn
	maxPublishSize int
	topicPrefix string
}

// NewFStompPublisherTransportFactoryBuilder creates a builder for
// FStompPublisherTransportFactories.
func NewFStompPublisherTransportFactoryBuilder(conn *stomp.Conn) *FStompPublisherTransportFactoryBuilder {
	return &FStompPublisherTransportFactoryBuilder{
		conn: conn,
	}
}

// WithMaxPublishSize allows setting the maximum size of a message this transport
// will allow to be published.
func (f *FStompPublisherTransportFactoryBuilder) WithMaxPublishSize(maxPublishSize int) *FStompPublisherTransportFactoryBuilder {
	f.maxPublishSize = maxPublishSize
	return f
}

// WithTopicPrefix allows setting a string to be added to the beginning of the
// constructed topic.
func (f *FStompPublisherTransportFactoryBuilder) WithTopicPrefix(topicPrefix string) *FStompPublisherTransportFactoryBuilder {
	f.topicPrefix = topicPrefix
	return f
}

// Build creates an FStompPublisherTransportFactory with the configured settings.
func (f *FStompPublisherTransportFactoryBuilder) Build() FPublisherTransportFactory {
	return newFStompPublisherTransportFactory(f.conn, f.maxPublishSize, f.topicPrefix)
}

// FStompPublisherTransportFactory creates fStompPublisherTransports.
type fStompPublisherTransportFactory struct {
	conn           *stomp.Conn
	maxPublishSize int
	topicPrefix    string
}

// NewFStompPublisherTransportFactory creates an FStompPublisherTransportFactory using the
// provided stomp connection.
func newFStompPublisherTransportFactory(conn *stomp.Conn, maxPublishSize int, topicPrefix string) *fStompPublisherTransportFactory {
	return &fStompPublisherTransportFactory{conn: conn, maxPublishSize: maxPublishSize, topicPrefix: topicPrefix}
}

// GetTransport creates a new stomp FPublisherTransport.
func (m *fStompPublisherTransportFactory) GetTransport() FPublisherTransport {
	return newStompFPublisherTransport(m.conn, m.maxPublishSize, m.topicPrefix)
}

// fStompPublisherTransport implements FPublisherTransport.
type fStompPublisherTransport struct {
	conn           *stomp.Conn
	maxPublishSize int
	topicPrefix    string
	isOpen bool
}

// newStompFPublisherTransport creates a new FPublisherTransport which is used for
// publishing using stomp protocol with scopes.
func newStompFPublisherTransport(conn *stomp.Conn, maxPublishSize int, topicPrefix string) FPublisherTransport {
	return &fStompPublisherTransport{conn: conn, maxPublishSize: maxPublishSize, topicPrefix: topicPrefix}
}

// Open initializes the transport.
func (m *fStompPublisherTransport) Open() error {
	if m.conn == nil {
		return thrift.NewTTransportException(TRANSPORT_EXCEPTION_NOT_OPEN, "frugal: stomp transport not open")
	}
	m.isOpen = true
	return nil
}

// IsOpen returns true if the transport is open, false otherwise.
func (m *fStompPublisherTransport) IsOpen() bool {
	return m.conn != nil && m.isOpen
}

// Close closes the transport.
func (m *fStompPublisherTransport) Close() error {
	m.isOpen = false
	return nil
}

// GetPublishSizeLimit returns the maximum allowable size of a payload
// to be published. 0 is returned to indicate an unbounded allowable size.
func (m *fStompPublisherTransport) GetPublishSizeLimit() uint {
	return uint(m.maxPublishSize)
}

// Publish sends the given payload with the transport.
func (m *fStompPublisherTransport) Publish(topic string, data []byte) error {
	if !m.IsOpen() {
		return thrift.NewTTransportException(TRANSPORT_EXCEPTION_NOT_OPEN, "frugal: stomp transport not open")
	}

	if m.maxPublishSize > 0 && len(data) > m.maxPublishSize {
		return thrift.NewTTransportException(
			TRANSPORT_EXCEPTION_REQUEST_TOO_LARGE,
			fmt.Sprintf("Message exceeds %d bytes, was %d bytes", m.maxPublishSize, len(data)))
	}

	destination := m.formatStompPublishTopic(topic)
	logger().Debugf("frugal: publishing stomp message on topic '%s'", destination)
	if err := m.conn.Send(destination, "application/octet-stream", data, stomp.SendOpt.Header("persistent", "true")); err != nil {
		return thrift.NewTTransportExceptionFromError(err)
	}
	logger().Debugf("frugal: finished publishing stomp message to '%s'", destination)
	return nil
}

func (m *fStompPublisherTransport) formatStompPublishTopic(topic string) string {
	return fmt.Sprintf("/topic/%s%s%s", m.topicPrefix, frugalPrefix, topic)
}

type FStompSubscriberTransportFactoryBuilder struct {
	conn *stomp.Conn
	topicPrefix string
	useQueue bool
}

func NewFStompSubscriberTransportFactoryBuilder(conn *stomp.Conn) *FStompSubscriberTransportFactoryBuilder {
	return &FStompSubscriberTransportFactoryBuilder{
		conn: conn,
		topicPrefix: "",
		useQueue: false,
	}
}

func (f *FStompSubscriberTransportFactoryBuilder) WithTopicPrefix(topicPrefix string) *FStompSubscriberTransportFactoryBuilder {
	f.topicPrefix = topicPrefix
	return f
}

func (f *FStompSubscriberTransportFactoryBuilder) WithUseQueues(useQueue bool) *FStompSubscriberTransportFactoryBuilder {
	f.useQueue = useQueue
	return f
}

func (f *FStompSubscriberTransportFactoryBuilder) Build() FSubscriberTransportFactory {
	return &fStompSubscriberTransportFactory{
		conn: f.conn,
		topicPrefix: f.topicPrefix,
		useQueue: f.useQueue,
	}
}

// fStompSubscriberTransportFactory creates fStompSubscriberTransports.
type fStompSubscriberTransportFactory struct {
	conn        *stomp.Conn
	topicPrefix string
	useQueue    bool
}

// newFStompSubscriberTransportFactory creates fStompSubscriberTransportFactory with the given stomp
// connection and consumer name.
func newFStompSubscriberTransportFactory(conn *stomp.Conn, topicPrefix string, useQueue bool) *fStompSubscriberTransportFactory {
	return &fStompSubscriberTransportFactory{conn: conn, topicPrefix: topicPrefix, useQueue: useQueue}
}

// GetTransport creates a new fStompSubscriberTransport.
func (m *fStompSubscriberTransportFactory) GetTransport() FSubscriberTransport {
	return newStompFSubscriberTransport(m.conn, m.topicPrefix, m.useQueue)
}

// fStompSubscriberTransport implements FSubscriberTransport.
type fStompSubscriberTransport struct {
	conn         *stomp.Conn
	topicPrefix  string
	topic        string
	useQueue     bool
	sub          *stomp.Subscription
	openMu       sync.RWMutex
	isSubscribed bool
	callback     FAsyncCallback
	stopC        chan bool
}

// newStompFSubscriberTransport creates a new FSubscriberTransport which is used for
// pub/sub.
func newStompFSubscriberTransport(conn *stomp.Conn, topicPrefix string, useQueue bool) FSubscriberTransport {
	return &fStompSubscriberTransport{conn: conn, topicPrefix: topicPrefix, useQueue: useQueue}
}

// Subscribe sets the subscribe topic and opens the transport.
//
// If an exception is raised by the provided callback, the message will
// not be acked with the broker. This behaviour allows the message to be
// redelivered and processing to be attempted again. If an exception is
// not raised by the provided callback, the message will be acked. This is
// used if processing succeeded, or if it's apparent processing will never
// succeed, as the message won't continue to be redelivered.
func (m *fStompSubscriberTransport) Subscribe(topic string, callback FAsyncCallback) error {
	m.openMu.Lock()
	defer m.openMu.Unlock()

	if m.conn == nil {
		return thrift.NewTTransportException(TRANSPORT_EXCEPTION_NOT_OPEN, "frugal: stomp transport not open")
	}

	if m.isSubscribed {
		return thrift.NewTTransportException(TRANSPORT_EXCEPTION_ALREADY_OPEN, "frugal: stomp transport already has a subscription")
	}

	if topic == "" {
		return thrift.NewTTransportException(TRANSPORT_EXCEPTION_UNKNOWN, "frugal: stomp transport cannot subscribe to empty topic")
	}

	var destination string
	if m.useQueue {
		destination = fmt.Sprintf("/queue/%s%s%s", m.topicPrefix, frugalPrefix, topic)
	} else {
		destination = fmt.Sprintf("/topic/%s%s%s", m.topicPrefix, frugalPrefix, topic)
	}

	sub, err := m.conn.Subscribe(destination, stomp.AckClientIndividual)
	if err != nil {
		return thrift.NewTTransportExceptionFromError(err)
	}
	m.stopC = make(chan bool, 1)
	m.sub = sub
	m.isSubscribed = true
	m.callback = callback
	m.topic = destination
	go m.processMessages()
	return nil
}

// IsSubscribed returns true if the transport is subscribed to a topic, false
// otherwise.
func (m *fStompSubscriberTransport) IsSubscribed() bool {
	m.openMu.RLock()
	defer m.openMu.RUnlock()
	return m.conn != nil && m.isSubscribed
}

// Unsubscribe unsubscribes from the destination.
func (m *fStompSubscriberTransport) Unsubscribe() error {
	m.openMu.Lock()
	defer m.openMu.Unlock()
	if !m.isSubscribed {
		logger().Info("frugal: unable to unsubscribe, subscription already unsubscribed")
		return nil
	}

	close(m.stopC)
	if err := m.sub.Unsubscribe(); err != nil {
		return thrift.NewTTransportExceptionFromError(err)
	}

	m.isSubscribed = false
	m.callback = nil
	return nil
}

// processMessages call the given FAsyncCallback with messages from the
// subscription channel.
func (m *fStompSubscriberTransport) processMessages() {
	stopC := m.stopC
	for {
		select {
		case <-stopC:
			logger().Errorf("frugal: error processing stomp subscription messages, message received on stop channel")
			return
		case message, ok := <-m.sub.C:
			logger().Debugf("frugal: received stomp message on topic '%s'", m.topic)
			if !ok {
				logger().Errorf("frugal: error processing subscription messages, message channel closed")
				return
			}

			if len(message.Body) < 4 {
				logger().Warnf("frugal: discarding invalid scope message frame, was length '%d'", len(message.Body))
				continue
			}

			transport := &thrift.TMemoryBuffer{Buffer: bytes.NewBuffer(message.Body[4:])}
			if err := m.callback(transport); err != nil {
				logger().Warn("frugal: error executing callback: ", err)
				continue
			}

			go m.ackMessage(message)
			logger().Debugf("frugal: finished processing stomp message from topic '%s'", m.topic)
		}
	}
}

// Acknowledges the stomp message.
func (m *fStompSubscriberTransport) ackMessage(message *stomp.Message) {
	if err := m.conn.Ack(message); err != nil {
		logger().WithError(err).Error("frugal: error acking stomp message")
	}
}
