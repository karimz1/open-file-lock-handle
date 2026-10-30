"""Last-resort headless RDP client: keep one loopback session signed in."""
import asyncio, os, sys
from aardwolf.commons.factory import RDPConnectionFactory
from aardwolf.commons.iosettings import RDPIOSettings

async def main() -> int:
    user, password, host = os.environ["RDP_USER"], os.environ["RDP_PASSWORD"], os.environ.get("RDP_HOST", "127.0.0.2")
    settings = RDPIOSettings()
    settings.video_width, settings.video_height = 1024, 768
    factory = RDPConnectionFactory.from_url(f"rdp+ntlm-password://{user}:{password}@{host}", settings)
    async with factory.create_connection_newtarget(host, settings) as conn:
        _, err = await conn.connect()
        if err is not None:
            print(f"aardwolf connect failed: {err!r}", file=sys.stderr)
            return 1
        print("aardwolf connected", flush=True)
        while True:  # drain frames so the server keeps the session alive
            await conn.ext_out_queue.get()

sys.exit(asyncio.run(main()))
