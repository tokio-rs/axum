#!/usr/bin/env python3
"""websocket client illustrating concurrency of async service"""

import argparse
import asyncio
import sys
import time

import aiohttp


async def start_client(url: str, n: int, datapipe: asyncio.Queue) -> None:

    async def dispatch(_ws: aiohttp.ClientWebSocketResponse) -> None:
        while True:
            msg = await _ws.receive()

            if msg.type == aiohttp.WSMsgType.TEXT:
                print(f">>>{n} Text: '{msg.data.strip()}'")
            elif msg.type == aiohttp.WSMsgType.BINARY:
                print(f">>>{n} Binary: '{msg.data}'")
            elif msg.type == aiohttp.WSMsgType.PING:
                print("PING WTF")
                await _ws.pong()
            elif msg.type == aiohttp.WSMsgType.PONG:
                print("Pong received")
            else:
                if msg.type == aiohttp.WSMsgType.CLOSE:
                    print("Got close!")
                    await _ws.close()
                    print("WS closed, exiting")
                    await datapipe.put(None)
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    print(f"{n}:Error during receive %s" % _ws.exception())
                elif msg.type == aiohttp.WSMsgType.CLOSED:
                    pass
                break

    async with aiohttp.ClientSession() as session:
        async with session.ws_connect(url, autoclose=True) as ws:
            # create a separate task to handle packets coming through websocket to us
            dispatch_task = asyncio.create_task(dispatch(ws))
            await ws.ping(b"Hello!")
            try:
                while x := await datapipe.get() is not None:
                    print(f"<<<{n} Text: '{x}'")
                    await ws.send_str(x)
                dispatch_task.cancel()
                await dispatch_task
            except asyncio.CancelledError:
                pass
            print(f"Client {n} exiting")


async def feed_stdin(tasks):
    try:
        # shamanic magic to get Python's asyncio to play nice with stdin
        loop = asyncio.get_event_loop()
        reader = asyncio.StreamReader()
        protocol = asyncio.StreamReaderProtocol(reader)
        await loop.connect_read_pipe(lambda: protocol, sys.stdin)
        # done!

        # Exit with Ctrl+D
        while line := await reader.readline():
            for t in tasks:
                await t[1].put(line)

        for t in tasks:
            await t[1].put(None)

        for t in tasks:
            t[0].cancel()
    except asyncio.CancelledError:
        pass


async def watch_clients(url, n):
    # list with task handles and datapipes to them
    tasks = []
    pipes = []
    # create a task for each of our clients
    for i in range(n):
        datapipe = asyncio.Queue()
        tasks.append(asyncio.create_task(start_client(url, i, datapipe)))
        pipes.append(datapipe)

    fs = asyncio.create_task(feed_stdin(tasks))

    # wait for clients to die
    await asyncio.gather(*tasks)
    fs.cancel()


if __name__ == "__main__":
    ARGS = argparse.ArgumentParser(
        description="websocket console client for axum websocket example."
    )
    ARGS.add_argument(
        "--url", default="http://127.0.0.1:3000/ws", help="Address to connect to"
    )
    ARGS.add_argument(
        "-n", default=2, help="Number of concurrent sessions"
    )
    args = ARGS.parse_args()
    print(f"Connecting to {args.url} with {args.n} sessions")
    print("""
    Type anything to send a Text message (from all connecitons)
    ctrl-D to exit early.
    Total time to run should not change whatsoever and be around 6.5 seconds.
    """)
    t1 = time.time()
    asyncio.run(watch_clients(args.url, args.n))
    t2 = time.time()
    print(f"Total time taken: {t2-t1}s")
