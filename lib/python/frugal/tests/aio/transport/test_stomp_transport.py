# Copyright 2017 Workiva
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#     http://www.apache.org/licenses/LICENSE-2.0
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import mock

from thrift.transport.TTransport import TTransportException

from frugal.aio.transport import FStompPublisherTransportFactory
from frugal.aio.transport import FStompSubscriberTransportFactory
from frugal.exceptions import TTransportExceptionType
from frugal.tests.aio import utils


class TestFStompPublisherTransport(utils.AsyncIOTestCase):
    def setUp(self, gen=None):
        super().setUp()
        self.mock_stomp_client = mock.Mock()
        pub_factory = FStompPublisherTransportFactory(self.mock_stomp_client)
        self.pub_trans = pub_factory.get_transport()

    def test_stomp_publisher_default_max_msg_size(self):
        self.assertEqual(
            self.pub_trans.get_publish_size_limit(), 32 * 1024 * 1024)

    @utils.async_runner
    async def test_stomp_publisher_publish_successfully(self):
        await self.pub_trans.open()
        data = bytearray([0, 0, 5, 2, 3, 4, 5, 6])
        self.mock_stomp_client.send.return_value = None
        await self.pub_trans.publish('foo', data)

        self.mock_stomp_client.send.assert_called_once_with(
            '/topic/frugal.foo',
            data,
            headers={'persistent': 'true',
                     'content-type': 'application/octet-stream'}
        )

    @utils.async_runner
    async def test_stomp_publisher_fails_publishing_payload_too_large(self):
        await self.pub_trans.open()
        mock_check_publish_size = mock.Mock(return_value=True)
        self.pub_trans._check_publish_size = mock_check_publish_size
        with self.assertRaises(TTransportException) as cm:
            await self.pub_trans.publish('foo', bytearray([0, 0, 5, 2, 3]))
        self.assertEqual(
            TTransportExceptionType.REQUEST_TOO_LARGE, cm.exception.type)


class TestFStompSubscriberTransport(utils.AsyncIOTestCase):
    def setUp(self, gen=None):
        super().setUp()
        self.mock_stomp_client = mock.Mock()
        sub_factory = FStompSubscriberTransportFactory(
            stomp_client=self.mock_stomp_client,
            topic_prefix='Consumer.foo.',
            use_queue=True)
        self.sub_trans = sub_factory.get_transport()

    @utils.async_runner
    async def test_stomp_subscriber_already_subscribed(self):
        self.sub_trans._sub = 'not none'
        with self.assertRaises(TTransportException) as cm:
            await self.sub_trans.subscribe('bar', None)
        self.assertEqual(TTransportException.ALREADY_OPEN, cm.exception.type)

    @utils.async_runner
    async def test_stomp_subscriber_empty_topic(self):
        with self.assertRaises(TTransportException) as cm:
            await self.sub_trans.subscribe('', None)
        self.assertEqual(TTransportException.UNKNOWN, cm.exception.type)

    @utils.async_runner
    async def test_stomp_subscriber_subscribe(self):
        mock_cb = mock.Mock()
        await self.sub_trans.subscribe('topic', mock_cb)
        self.mock_stomp_client.subscribe.assert_called_once_with(
            '/queue/Consumer.foo.frugal.topic',
            ack='client-individual',
            handler=mock.ANY
        )
        self.assertTrue(self.sub_trans.is_subscribed())

    @utils.async_runner
    async def test_stomp_subscriber_unsubscribe(self):
        mock_sub = mock.Mock()
        self.sub_trans._sub = mock_sub
        self.mock_stomp_client.unsubscribe.return_value = None
        await self.sub_trans.unsubscribe()

        self.mock_stomp_client.unsubscribe.assert_called_once_with(mock_sub)
        self.assertFalse(self.sub_trans.is_subscribed())

    @utils.async_runner
    async def test_stomp_subscriber_unsubscribe_already_unsubscribed(self):
        await self.sub_trans.unsubscribe()
        self.assertFalse(self.mock_stomp_client.called)
