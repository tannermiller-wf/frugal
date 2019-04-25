import argparse
import asyncio
import sys

from aiostomp import AioStomp

sys.path.append('gen_py_asyncio')
sys.path.append('..')

from frugal.context import FContext
from frugal.provider import FScopeProvider

from frugal.aio.transport import (
    FStompPublisherTransportFactory,
    FStompSubscriberTransportFactory
)

from frugal_test.f_Events_publisher import EventsPublisher
from frugal_test.ttypes import Xception, Insanity, Xception2, Event
from frugal_test.f_Events_subscriber import EventsSubscriber

from common.utils import *

response_received = False


async def main():
    global response_received
    parser = argparse.ArgumentParser(
        description='Run a python asyncio stomp publisher')
    parser.add_argument('--port', dest='port', default='9090')
    parser.add_argument('--protocol', dest='protocol_type', default='binary',
                        choices='binary, compact, json')
    parser.add_argument('--transport', dest='transport_type',
                        default=ACTIVEMQ_NAME, choices='activemq')
    args = parser.parse_args()

    protocol_factory = get_protocol_factory(args.protocol_type)

    if args.transport_type == ACTIVEMQ_NAME:
        stomp_client = AioStomp('localhost', 61613)
        await stomp_client.connect()

        pub_transport_factory = FStompPublisherTransportFactory(stomp_client)
        sub_transport_factory = FStompSubscriberTransportFactory(stomp_client)
        provider = FScopeProvider(
            pub_transport_factory, sub_transport_factory, protocol_factory)
        publisher = EventsPublisher(provider)
    else:
        print(
            'Unknown transport type: {type}'.format(type=args.transport_type))
        sys.exit(1)

    await publisher.open()

    def subscribe_handler(context, event):
        print('Response received {}'.format(event))
        global response_received
        if context:
            response_received = True

    # Subscribe to response
    preamble = 'foo'
    ramble = 'bar'
    subscriber = EventsSubscriber(provider)
    await subscriber.subscribe_EventCreated(preamble, ramble, 'response',
                                            '{}'.format(args.port),
                                            subscribe_handler)

    event = Event(Message='Sending Call')
    context = FContext('Call')
    context.set_request_header(PREAMBLE_HEADER, preamble)
    context.set_request_header(RAMBLE_HEADER, ramble)
    print('Publishing...')
    await publisher.publish_EventCreated(context, preamble, ramble, 'call',
                                         '{}'.format(args.port), event)

    # Loop with sleep interval. Fail if not received within 3 seconds
    total_time = 0
    interval = 0.1
    while total_time < 3:
        if response_received:
            break
        else:
            await asyncio.sleep(interval)
            total_time += interval

    if not response_received:
        print('publisher did not get a response from subscriber!')
        exit(1)

    await publisher.close()
    exit(0)


if __name__ == '__main__':
    io_loop = asyncio.get_event_loop()
    io_loop.run_until_complete(main())
