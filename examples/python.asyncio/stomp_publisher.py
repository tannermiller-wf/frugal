#!/usr/bin/python
# -*- coding: utf-8 -*-
import asyncio
import logging
import os
import sys
import uuid

from aiostomp import AioStomp
from thrift.protocol import TBinaryProtocol

from frugal.protocol import FProtocolFactory
from frugal.provider import FScopeProvider
from frugal.context import FContext
from frugal.aio.transport import FStompPublisherTransportFactory

sys.path.append(os.path.join(os.path.dirname(__file__), "gen-py.asyncio"))
from v1.music.f_AlbumWinners_publisher import AlbumWinnersPublisher  # noqa
from v1.music.ttypes import Album, Track, PerfRightsOrg  # noqa


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
    transport_factory = FStompPublisherTransportFactory(stomp_client)
    provider = FScopeProvider(transport_factory, None, prot_factory)

    # Create a publisher
    publisher = AlbumWinnersPublisher(provider)
    await publisher.open()

    # Publish an album win event
    album = Album()
    album.ASIN = str(uuid.uuid4())
    album.duration = 12000
    album.tracks = [Track(title="Comme des enfants",
                          artist="Coeur de pirate",
                          publisher="Grosse Boîte",
                          composer="Béatrice Martin",
                          duration=169,
                          pro=PerfRightsOrg.ASCAP)]
    await publisher.publish_Winner(FContext(), album)
    await publisher.publish_ContestStart(FContext(), [album, album])

    # Close publisher and nats client
    await publisher.close()
    stomp_client.close()


if __name__ == '__main__':
    io_loop = asyncio.get_event_loop()
    io_loop.run_until_complete(main())
