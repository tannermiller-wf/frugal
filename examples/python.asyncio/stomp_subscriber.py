#!/usr/bin/python
# -*- coding: utf-8 -*-
import asyncio
import logging
import os
import sys

from aiostomp import AioStomp
from thrift.protocol import TBinaryProtocol

from frugal.protocol import FProtocolFactory
from frugal.provider import FScopeProvider
from frugal.aio.transport import FStompSubscriberTransportFactory

sys.path.append(os.path.join(os.path.dirname(__file__), "gen-py.asyncio"))
from v1.music.f_AlbumWinners_subscriber import AlbumWinnersSubscriber  # noqa



root = logging.getLogger()
root.setLevel(logging.DEBUG)

ch = logging.StreamHandler(sys.stdout)
ch.setLevel(logging.DEBUG)
formatter = logging.Formatter(
    '%(asctime)s - %(levelname)s - %(message)s')
ch.setFormatter(formatter)
root.addHandler(ch)


async def main():
    # Declare the protocol stack used for serialization.
    # Protocol stacks must match between publishers and subscribers.
    prot_factory = FProtocolFactory(TBinaryProtocol.TBinaryProtocolFactory())

    # Open a aiostomp connection, default stomp port in activemq is 61613
    stomp_client = AioStomp('localhost', 61613)
    await stomp_client.connect()

    # Create a pub sub scope using the configured transport and protocol
    transport_factory = FStompSubscriberTransportFactory(stomp_client)
    provider = FScopeProvider(None, transport_factory, prot_factory)

    subscriber = AlbumWinnersSubscriber(provider)

    def event_handler(ctx, req):
        raise ValueError('BLAH')
        root.info("You won! {}".format(req.ASIN))

    def start_contest_handler(ctx, albums):
        root.info("Contest started, available albums: {}".format(albums))

    await subscriber.subscribe_Winner(event_handler)
    await subscriber.subscribe_ContestStart(start_contest_handler)

    root.info("Subscriber starting...")

if __name__ == '__main__':
    io_loop = asyncio.get_event_loop()
    asyncio.ensure_future(main())
    io_loop.run_forever()
