import argparse
import asyncio
import http
import socketserver
import sys
import threading

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


message_received = False


async def main():
    global message_received
    parser = argparse.ArgumentParser(
        description='Run a python asyncio stomp publisher')
    parser.add_argument('--port', dest='port', default='9090')
    parser.add_argument('--protocol', dest='protocol_type', default='binary',
                        choices="binary, compact, json")
    parser.add_argument('--transport', dest='transport_type',
                        default=ACTIVEMQ_NAME, choices='activemq')
    args = parser.parse_args()

    protocol_factory = get_protocol_factory(args.protocol_type)

    if args.transport_type == ACTIVEMQ_NAME:
        stomp_client = AioStomp('localhost', 61613)
        await stomp_client.connect()

        pub_transport_factory = FStompPublisherTransportFactory(stomp_client)
        sub_transport_factory = FStompSubscriberTransportFactory(stomp_client)
    else:
        print(
            'Unknown transport type: {type}'.format(type=args.transport_type))
        sys.exit(1)

    provider = FScopeProvider(
        pub_transport_factory, sub_transport_factory, protocol_factory)

    # start healthcheck so the test runner knows the server is running
    threading.Thread(target=healthcheck,
                     args=(args.port,)
                     ).start()

    async def subscribe_handler(context, event):
        publisher = EventsPublisher(provider)
        try:
            await publisher.open()
            preamble = context.get_request_header(PREAMBLE_HEADER)
            if preamble is None or preamble == '':
                print('Client did not provide preamble header')
                return
            ramble = context.get_request_header(RAMBLE_HEADER)
            if ramble is None or ramble == '':
                print('Client did not provide ramble header')
                return
            response_event = Event(Message='Sending Response')
            response_context = FContext('Call')
            await publisher.publish_EventCreated(
                response_context, preamble, ramble, 'response',
                '{}'.format(args.port), response_event)
            global message_received
            message_received = True
        except Exception as e:
            print('Error opening publisher to respond:' + repr(e))

    subscriber = EventsSubscriber(provider)
    await subscriber.subscribe_EventCreated(
        '*', '*', 'call', '{}'.format(args.port), subscribe_handler)

    # Loop with sleep interval. Fail if not received within 3 seconds
    total_time = 0
    interval = 0.1
    while total_time < 3:
        if message_received:
            break
        else:
            await asyncio.sleep(interval)
            total_time += interval

    if not message_received:
        print('subscriber did not received message!')
        exit(1)

    exit(0)


def healthcheck(port):
    health_handler = http.server.SimpleHTTPRequestHandler
    healthcheck = socketserver.TCPServer(('', int(port)), health_handler)
    healthcheck.serve_forever()


if __name__ == '__main__':
    io_loop = asyncio.get_event_loop()
    io_loop.run_until_complete(main())
